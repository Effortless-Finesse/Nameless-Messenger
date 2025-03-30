use std::{
    collections::HashMap,
    io::{self, BufReader, BufRead, Write, Read},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

type ClientList = Arc<Mutex<HashMap<String, TcpStream>>>;

fn msg_fetcher(stream: TcpStream, clients: ClientList) {
    // Fetch the peer's address (the address of the client)
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(_) => {
            eprintln!("Could not fetch peer address");
            return;
        }
    };

    let mut reader = BufReader::new(&stream); // Buffer that stores incoming messages

    // Add client to the client list
    if let Err(_) = add_client_to_list(&clients, peer.clone(), &stream) {
        eprintln!("Error while adding client to list");
        return;
    }

    let mut buffer = String::new(); // Buffer between stream and output

    loop {
        // Read the message from the client
        match reader.read_line(&mut buffer) {
            Ok(0) | Err(_) | Ok(_) if buffer.trim().is_empty() => {
                break; // Exit loop if reading fails or the buffer is empty
            }
            Ok(_) => {
                let message = format!("{} : {}", peer, buffer.trim()); // Format of message: "<client address> : <message>"
                println!("{}", message);

                // Broadcast the message to other clients
                if let Err(_) = broadcast_message(&clients, &peer, &message) {
                    eprintln!("Error broadcasting message");
                    break;
                }

                buffer.clear(); // Clear the buffer after processing the message
            }
        }
    }

    // Disconnect the client once the loop ends
    if let Err(_) = disconnect_client(&clients, peer) {
        eprintln!("Error removing client from list");
    }
}

// Function to add the client to the list of clients
fn add_client_to_list(clients: &ClientList, peer: std::net::SocketAddr, stream: &TcpStream) -> io::Result<()> {
    let mut clients_lock = clients.lock().unwrap();
    clients_lock.insert(peer.to_string(), stream.try_clone()?);
    Ok(())
}

// Function to broadcast a message to all clients except the sender
fn broadcast_message(clients: &ClientList, sender: &std::net::SocketAddr, message: &str) -> io::Result<()> {
    let clients_lock = clients.lock().unwrap();

    for (client_addr, client_stream) in clients_lock.iter() {
        if client_addr != &sender.to_string() {
            // Send the message to all clients except the sender
            writeln!(mut client_stream, "{}", message)?;
        }
    }
    Ok(())
}

// Function to disconnect the client from the client list
fn disconnect_client(clients: &ClientList, peer: std::net::SocketAddr) -> io::Result<()> {
    let mut clients_lock = clients.lock().unwrap();
    if let Some(_) = clients_lock.remove(&peer.to_string()) {
        println!("Client {} has disconnected", peer);
    }
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
