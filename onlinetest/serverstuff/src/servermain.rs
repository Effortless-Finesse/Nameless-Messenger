use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    thread,
};
use aes_gcm::{Key, Aes256Gcm};
use rand::Rng;
use std::io::Read;


type SharedStream = Arc<TcpStream>;
type ClientList = Arc<Mutex<HashMap<String, SharedStream>>>; // username -> stream
type AESKey = [u8; 32];

fn get_ip() -> String {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Error binding to socket for IP detection");
    socket.connect("8.8.8.8:80").expect("Error connecting to dummy address for IP detection");
    socket.local_addr().unwrap().ip().to_string()
}

fn generate_aes_key() -> AESKey {
    let mut rng = rand::thread_rng();
    let mut key = [0u8; 32];
    rng.fill(&mut key);
    key
}

fn msg_fetcher(stream: TcpStream, clients: ClientList, aes_key: Arc<AESKey>) {
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(_) => {
            eprintln!("Could not fetch peer address");
            return;
        }
    };

    let stream = Arc::new(stream);

    let intro_stream = match TcpStream::try_clone(&stream) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to clone stream for intro reading: {}", e);
            return;
        }
    };

    let reader_stream = match TcpStream::try_clone(&stream) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to clone stream: {}", e);
            return;
        }
    };
    
    let mut reader = BufReader::new(reader_stream);
    let mut intro_reader = BufReader::new(intro_stream);
    let mut intro = String::new();
    if intro_reader.read_line(&mut intro).is_err() {
        eprintln!("Failed to read intro message from client");
        return;
    }
    let username = parse_username(&intro).unwrap_or_else(|| peer.to_string());

    if let Err(e) = add_client_to_list(&clients, username.clone(), Arc::clone(&stream)) {
        eprintln!("Error adding client: {}", e);
        return;
    }
    // stream.write_all(aes_key.as_ref())
    
    let mut raw_stream = match TcpStream::try_clone(&stream) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to clone stream for sending AES key: {}", e);
            return;
        }
    };
    
    if let Err(e) = raw_stream.write_all(aes_key.as_ref()) {
        eprintln!("Failed to send AES key to {}: {}", username, e);
        return;
    }
    
    // let mut reader = BufReader::new(reader_stream);
    // let mut buffer = String::new();

    loop {
        // buffer.clear();
        let mut nonce_bytes = [0u8; 12];
        if reader.read_exact(&mut nonce_bytes).is_err() {
            break;
        }
        let mut size_buf = [0u8; 2];
        if reader.read_exact(&mut size_buf).is_err() {
            break;
        }
        let size = u16::from_be_bytes(size_buf) as usize;
        let mut ciphertext = vec![0u8; size];
        if reader.read_exact(&mut ciphertext).is_err() {
            break;
        }
        let mut message = Vec::with_capacity(12 + 2 + size);
        message.extend_from_slice(&nonce_bytes);
        message.extend_from_slice(&size_buf);
        message.extend_from_slice(&ciphertext);
        if let Err(e) = broadcast_message(&clients, &username, &message) {
            eprintln!("Broadcast error: {}", e);
            break;
        }
        // match reader.read_line(&mut buffer) {
        //     Ok(0) => break,
        //     Ok(_) if buffer.trim().is_empty() => break,
        //     Ok(_) => {
        //         let message = format!("{}",buffer.trim());
        //         print!("{}", message);
        //         // println!("{}", message);

        //         if let Err(e) = broadcast_message(&clients, &username, &message) {
        //             eprintln!("Broadcast error: {}", e);
        //             break;
        //         }
        //     }
        //     Err(e) => {
        //         eprintln!("Read error from {}: {}", peer, e);
        //         break;
        //     }
        // }
    }

    // if let Err(e) = disconnect_client(&clients, &username, Arc::clone(&stream)) {
    //     eprintln!("Error removing client: {}", e);
    // }
}

fn parse_username(intro: &str) -> Option<String> {
    let parts: Vec<&str> = intro.trim().split_whitespace().collect();
    if parts.len() == 2 && parts[0] == "client" {
        Some(parts[1].to_string())
    } else {
        None
    }
}

fn add_client_to_list(clients: &ClientList, username: String, stream: SharedStream) -> io::Result<()> {
    let mut clients_lock = clients.lock().unwrap();
    clients_lock.insert(username, stream);
    Ok(())
}

fn broadcast_message(clients: &ClientList, sender_username: &str, message: &[u8]) -> io::Result<()> {
    let mut disconnected_clients = vec![];
    let clients_lock = clients.lock().unwrap();

    for (username, client_stream) in clients_lock.iter() {
        if username != sender_username {
            if let Ok(mut stream) = TcpStream::try_clone(&**client_stream) {
                if let Err(e) = stream.write_all(message) {
                    eprintln!("Failed to send to {}: {}", username, e);
                    disconnected_clients.push(username.clone());
                }
            }
        }
    }
    drop(clients_lock);

    if !disconnected_clients.is_empty() {
        let mut clients_lock = clients.lock().unwrap();
        for username in disconnected_clients {
            clients_lock.remove(&username);
            println!("Removed disconnected client: {}", username);
        }
    }

    Ok(())
}

fn disconnect_client(clients: &ClientList, username: &str, stream: SharedStream) -> io::Result<()> {
    let mut clients_lock = clients.lock().unwrap();
    if clients_lock.remove(username).is_some() {
        println!("Client {} disconnected", username);
    }
    stream.shutdown(Shutdown::Both)?;
    Ok(())
}

fn main() -> io::Result<()> {
    print!("Enter lobby address: ");
    io::stdout().flush()?;
    let mut lobby_addr = String::new();
    io::stdin().read_line(&mut lobby_addr)?;
    let lobby_addr = lobby_addr.trim().to_string();

    print!("Choose a name for your server: ");
    io::stdout().flush()?;
    let mut serv_name = String::new();
    io::stdin().read_line(&mut serv_name)?;
    let serv_name = serv_name.trim().to_string();

    let serv_ip = get_ip();
    // let serv_ip = "0.tcp.eu.ngrok.io:14770";

    let mut to_lobby = TcpStream::connect(lobby_addr).expect("Could not connect to lobby");
    writeln!(to_lobby, "server {} {}", serv_ip, serv_name).expect("Failed to register with lobby");
    to_lobby.shutdown(Shutdown::Write).ok();

    let aes_key = Arc::new(generate_aes_key());

    let listener = TcpListener::bind("0.0.0.0:8081")?;
    let clients: ClientList = Arc::new(Mutex::new(HashMap::new()));

    println!("Server '{}' is running at {}:8081", serv_name, serv_ip);

    for stream in listener.incoming() {
        let stream = stream?;
        let clients = Arc::clone(&clients);
        let aes_key_clone = Arc::clone(&aes_key);

        thread::spawn(move || {
            msg_fetcher(stream, clients, aes_key_clone);
        });
    }

    Ok(())
}