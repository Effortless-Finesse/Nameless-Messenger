use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

// Crypto imports
use rand::Rng;
use rsa::{RsaPublicKey, Pkcs1v15Encrypt};
use aes_gcm::{Aes256Gcm, KeyInit, aead::{Aead, generic_array::GenericArray}};
use rand::rngs::OsRng;
use hex;

type SharedStream = Arc<TcpStream>;
type ClientList = Arc<Mutex<HashMap<String, SharedStream>>>;
type PubKeyStore = Arc<Mutex<HashMap<String, RsaPublicKey>>>;

struct ServerState {
    clients: ClientList,
    pubkeys: PubKeyStore,
    aes_key: Vec<u8>,
}

impl ServerState {
    fn new() -> Self {
        let mut rng = OsRng;
        let mut aes_key = vec![0u8; 32];
        rng.fill(&mut aes_key);
        
        ServerState {
            clients: Arc::new(Mutex::new(HashMap::new())),
            pubkeys: Arc::new(Mutex::new(HashMap::new())),
            aes_key,
        }
    }
}

fn handle_client(stream: TcpStream, state: Arc<ServerState>) {
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(_) => {
            eprintln!("Could not fetch peer address");
            return;
        }
    };

    let stream = Arc::new(stream);
    let reader_stream = match TcpStream::try_clone(&stream) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to clone stream: {}", e);
            return;
        }
    };

    let mut reader = BufReader::new(reader_stream);
    let mut intro = String::new();
    if reader.read_line(&mut intro).is_err() {
        eprintln!("Failed to read intro message");
        return;
    }

    let username = if intro.starts_with("client ") {
        intro[7..].trim().to_string()
    } else if intro.starts_with("PUBKEY ") {
        let parts: Vec<&str> = intro[7..].trim().splitn(2, ' ').collect();
        if parts.len() == 2 {
            if let Ok(pubkey) = RsaPublicKey::from_pkcs1_pem(parts[1]) {
                state.pubkeys.lock().unwrap().insert(parts[0].to_string(), pubkey);
                
                // Send encrypted AES key
                let encrypted_key = pubkey.encrypt(&mut OsRng, Pkcs1v15Encrypt, &state.aes_key).unwrap();
                if writeln!(&*stream, "AESKEY {}", hex::encode(encrypted_key)).is_err() {
                    eprintln!("Failed to send AES key");
                }
            }
            parts[0].to_string()
        } else {
            peer.to_string()
        }
    } else {
        peer.to_string()
    };

    // Add client to active connections
    state.clients.lock().unwrap().insert(username.clone(), Arc::clone(&stream));

    let mut buffer = String::new();
    loop {
        buffer.clear();
        match reader.read_line(&mut buffer) {
            Ok(0) => break, // Connection closed
            Ok(_) => {
                let msg = buffer.trim();
                if !msg.is_empty() {
                    broadcast_message(&state, &username, msg);
                }
            }
            Err(e) => {
                eprintln!("Read error from {}: {}", username, e);
                break;
            }
        }
    }

    // Clean up
    state.clients.lock().unwrap().remove(&username);
    state.pubkeys.lock().unwrap().remove(&username);
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

fn broadcast_message(state: &Arc<ServerState>, sender: &str, message: &str) {
    let clients = state.clients.lock().unwrap();
    let mut disconnected = Vec::new();

    for (username, stream) in clients.iter() {
        if username != sender {
            if writeln!(&**stream, "{}", message).is_err() {
                disconnected.push(username.clone());
            }
        }
    }

    if !disconnected.is_empty() {
        let mut clients = state.clients.lock().unwrap();
        for username in disconnected {
            clients.remove(&username);
            state.pubkeys.lock().unwrap().remove(&username);
            println!("Client {} disconnected", username);
        }
    }
}

fn main() -> io::Result<()> {
    println!("Starting server...");
    
    let state = Arc::new(ServerState::new());
    let listener = TcpListener::bind("0.0.0.0:8081")?;
    
    println!("Server listening on port 8081");
    println!("Shared AES key: {}", hex::encode(&state.aes_key));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    handle_client(stream, state);
                });
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }

    Ok(())
}