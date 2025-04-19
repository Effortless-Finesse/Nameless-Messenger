use std::{
    collections::HashMap,
    io::{Read, Write, BufReader, BufRead},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    thread,
};

type ServerList = Arc<Mutex<HashMap<String, String>>>; // ip -> name

fn get_ip() -> String {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Error binding to socket for IP detection");
    socket.connect("8.8.8.8:80").expect("Error connecting to dummy address for IP detection");
    socket.local_addr().unwrap().ip().to_string()
}

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").expect("Could not bind listener");
    let servers: ServerList = Arc::new(Mutex::new(HashMap::new()));

    let ip_addr : String = get_ip();
    println!("IP acdress of this lobby: {}", ip_addr);

    for stream in listener.incoming() {
        let servers = Arc::clone(&servers); // Clone Arc for this thread
        let mut stream = stream.expect("Failed to accept connection");

        thread::spawn(move || {
            let mut reader = BufReader::new(&stream);
            let mut request = String::new();
            reader.read_line(&mut request).expect("Could not read from stream");

            if request.starts_with("server") {
                let (ip, name) = get_server_info(&request);
                add_server(servers, ip, name);
            } else if request.starts_with("client") {
                let indexed_list = send_server_list(&mut stream, &servers);
                handle_client_reply(&mut stream, &indexed_list);
            }
        });
    }
}

/// Parses a request string and extracts (ip, name)
fn get_server_info(request: &str) -> (String, String) {
    // Expected format: "server <ip> <name>"
    let parts: Vec<&str> = request.trim().split_whitespace().collect();
    if parts.len() < 3 {
        return ("unknown".to_string(), "unnamed".to_string());
    }
    (parts[1].to_string(), parts[2..].join(" "))
}

/// Adds a new server to the shared list
fn add_server(servers: ServerList, ip: String, name: String) {
    let mut servers_lock = servers.lock().unwrap();
    servers_lock.insert(ip.clone(), name.clone());
    println!("Registered server '{}' at {}", name, ip);
}

/// Sends the list of available servers to the client and returns it as a Vec<(ip, name)>
fn send_server_list(stream: &mut TcpStream, servers: &ServerList) -> Vec<(String, String)> {
    let servers_lock = servers.lock().unwrap();
    let mut response = String::new();
    let mut indexed_list = vec![];

    for (i, (ip, name)) in servers_lock.iter().enumerate() {
        response.push_str(&format!("{}. {} - {}\n", i + 1, name, ip));
        indexed_list.push((ip.clone(), name.clone()));
    }

    if response.is_empty() {
        response.push_str("No servers available.\n");

    }
    else{
        response.push_str("\n");
    }

    stream.write_all(response.as_bytes()).expect("Failed to send server list");

    indexed_list
}

/// Receives a reply from the client with the server they want
fn handle_client_reply(stream: &mut TcpStream, indexed_list: &Vec<(String, String)>) {
    let mut buffer = [0; 128];
    if let Ok(size) = stream.read(&mut buffer) {
        let selection = String::from_utf8_lossy(&buffer[..size]).trim().to_string();

        if let Ok(index) = selection.parse::<usize>() {
            if index > 0 && index <= indexed_list.len() {
                let (ip, name) = &indexed_list[index - 1];
                let msg = format!("{}:8081\n", ip);
                stream.write_all(msg.as_bytes()).expect("Failed to send connection info");
                println!("Client selected {} ({})", name, ip);

                return;
            }
        }

        // If we reach here, the selection was invalid
        let error_msg = "Invalid selection. Please restart and choose a valid server.\n";
        stream.write_all(error_msg.as_bytes()).expect("Failed to send error");
    }
}
