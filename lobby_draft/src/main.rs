use std::{
    collections::HashMap,
    io::{Read, Write, BufReader, BufRead},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    thread,
};
//this lobby
type ServerList = Arc<Mutex<HashMap<String, String>>>; // ip -> name

fn get_ip() -> String {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket for IP detection");
    socket.connect("8.8.8.8:80").expect("Failed to connect to external address");
    socket.local_addr().unwrap().ip().to_string()
}

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").expect("Could not bind listener");
    let servers: ServerList = Arc::new(Mutex::new(HashMap::new()));

    let ip_addr = get_ip();
    println!("IP address of this lobby: {}:8080", ip_addr);

    for stream in listener.incoming() {
        let servers = Arc::clone(&servers);
        let mut stream = stream.expect("Failed to accept connection");

        thread::spawn(move || {
            let mut reader = BufReader::new(&stream);
            let mut request = String::new();
            if reader.read_line(&mut request).is_err() {
                eprintln!("Failed to read from client");
                return;
            }

            if request.starts_with("server") {
                let (ip, name) = get_server_info(&request);
                add_server(servers, ip, name);
            } else if request.starts_with("client") {
                if let Err(e) = send_server_address(&mut stream, &servers) {
                    eprintln!("Failed to send server address: {}", e);
                }
            }
        });
    }
}

fn get_server_info(request: &str) -> (String, String) {
    // Format: "server <ip> <name>"
    let parts: Vec<&str> = request.trim().split_whitespace().collect();
    if parts.len() < 3 {
        return ("unknown".to_string(), "unnamed".to_string());
    }
    (parts[1].to_string(), parts[2..].join(" "))
}

fn add_server(servers: ServerList, ip: String, name: String) {
    let mut servers_lock = servers.lock().unwrap();
    servers_lock.insert(ip.clone(), name.clone());
    println!("Registered server '{}' at {}", name, ip);
}

fn send_server_address(stream: &mut TcpStream, servers: &ServerList) -> std::io::Result<()> {
    let servers_lock = servers.lock().unwrap();
    if let Some((ip, _name)) = servers_lock.iter().next() {
        let full_address = format!("{}:8081\n", ip);
        stream.write_all(full_address.as_bytes())?;
    } else {
        stream.write_all(b"No servers available\n")?;
    }
    Ok(())
}
