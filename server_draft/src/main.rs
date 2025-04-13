use std::{
    collections::HashMap,
    io::{self, BufReader, BufRead, Write},
    net::{TcpListener, TcpStream, Shutdown, SocketAddr},
    sync::{Arc, Mutex},
    thread,
};

type SharedStream = Arc<TcpStream>;
type ClientList = Arc<Mutex<HashMap<String, SharedStream>>>;

fn msg_fetcher(stream: TcpStream, clients: ClientList) {
    // Fetch the peer's address (the address of the client)
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(_) => {
            eprintln!("Could not fetch peer address");
            return;
        }
    };

    let stream = Arc::new(stream);

    // Clone the stream for reading
    let reader_stream = match TcpStream::try_clone(&stream) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to clone stream for reading: {}", e);
            return
        }
    };

    let mut reader = BufReader::new(reader_stream); // Buffer that stores incoming messages

    // Add client to the client list
    if let Err(e) = add_client_to_list(&clients, peer, Arc::clone(&stream)) {
        eprintln!("Error while adding client to list: {}", e);
        return
    }

    let mut buffer = String::new(); // Buffer between stream and output

    loop {
        buffer.clear();
        match reader.read_line(&mut buffer) {
            Ok(0) => break, // Connection closed
            Ok(_) if buffer.trim().is_empty() => break,
            Ok(_) => {
                let message = format!("{} : {}", peer, buffer.trim()); // Format of message: "<client address> : <message>"
                println!("{}", message);

                // Broadcast the message to other clients
                if let Err(e) = broadcast_message(&clients, &peer, &message) {
                    eprintln!("Error broadcasting message: {}", e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error reading from client {}: {}", peer, e);
                break;
            }
        }
    }

    // Disconnect the client once the loop ends
    if let Err(e) = disconnect_client(&clients, peer, Arc::clone(&stream)) {
        eprintln!("Error removing client from list: {}", e);
    }
}

// Function to add the client to the list of clients
fn add_client_to_list(clients: &ClientList, peer: SocketAddr, stream: SharedStream) -> io::Result<()> {
    let mut clients_lock = clients.lock().unwrap();
    clients_lock.insert(peer.to_string(), stream);
    Ok(())
}

// Function to broadcast a message to all clients except the sender
fn broadcast_message(clients: &ClientList, sender: &SocketAddr, message: &str) -> io::Result<()> {
    let mut disconnected_clients = vec![];

    let clients_lock = clients.lock().unwrap();

    for (client_addr, client_stream) in clients_lock.iter() {
        if client_addr != &sender.to_string() {
            let mut stream = &**client_stream; // Deref Arc -> TcpStream -> &TcpStream
            if let Err(e) = writeln!(stream, "{}", message) {
                eprintln!("Failed to send message to {}: {}", client_addr, e);
                disconnected_clients.push(client_addr.clone());
            }
        }
    }
    drop(clients_lock);

    // Clean up disconnected clients
    if !disconnected_clients.is_empty() {
        let mut clients_lock = clients.lock().unwrap();
        for addr in disconnected_clients {
            clients_lock.remove(&addr);
            println!("Cleaned up disconnected client: {}", addr);
        }
    }

    Ok(())
}

// Function to disconnect the client from the client list
fn disconnect_client(clients: &ClientList, peer: SocketAddr, stream: SharedStream) -> io::Result<()> {
    let mut clients_lock = clients.lock().unwrap();
    if let Some(_) = clients_lock.remove(&peer.to_string()) {
        println!("Client {} has disconnected", peer);
    }

    stream.shutdown(Shutdown::Both)?;
    Ok(())
}

fn main() -> io::Result<()> {
    // Bind the server to localhost:8080
    let listener = TcpListener::bind("localhost:8080")?;

    // Initialize an empty list of clients
    let clients: ClientList = Arc::new(Mutex::new(HashMap::new()));

    // Accept incoming client connections and spawn a thread for each
    for stream in listener.incoming() {
        let stream = stream?;
        let clients = Arc::clone(&clients);

        // Spawn a new thread for each client connection
        thread::spawn(move || {
            msg_fetcher(stream, clients);
        });
    }

    Ok(())
}
