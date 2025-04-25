use std::{
    io::{self, BufRead, BufReader, Write, Read},
    net::TcpStream,
    sync::mpsc,
    thread,
};
use aes_gcm::{Aes256Gcm, Key, Nonce}; // AES cipher
use aes_gcm::aead::{Aead};
use aes_gcm::KeyInit;  
use rand::RngCore;
use std::sync::Arc;

fn main() -> io::Result<()> {
    // Read the username sent from the GTK UI via stdin
    eprintln!("Started");
    let mut stdin_reader = BufReader::new(io::stdin());
    let mut username = String::new();
    stdin_reader.read_line(&mut username)?;
    let username = username.trim().to_string();
    eprintln!("Username received: '{}'", username);


    // // Connect to the lobby
    let mut lobby_stream = TcpStream::connect("localhost:8080")?;
    // let mut lobby_stream = TcpStream::connect("5.tcp.eu.ngrok.io:18940")?;
    write!(lobby_stream, "client {}\n", username)?;
    lobby_stream.flush()?; // Ensure data is sent

    // Read server IP from lobby
    let mut buffer = [0u8; 128];
    let size = lobby_stream.read(&mut buffer)?;
    let target_ip = String::from_utf8_lossy(&buffer[..size]).trim().to_string();
    
    // let target_ip = "5.tcp.eu.ngrok.io:18940";

    // Connect directly to the chosen server
    let mut server_stream = TcpStream::connect(target_ip)?;
    write!(server_stream, "client {}\n", username)?;
    server_stream.flush()?;

    let mut key_bytes = [0u8; 32];
    server_stream.read_exact(&mut key_bytes)?; // Exactly 32 bytes
    let aes_cipher = Aes256Gcm::new(&key_bytes.into());
    eprintln!("Received AES key and initialized cipher.");

    let aes_cipher = Arc::new(aes_cipher);


    let read_stream = server_stream.try_clone()?;
    let write_stream = server_stream.try_clone()?;

    // Channel for UI input
    let (tx, rx) = mpsc::channel::<String>();

    let aes_cipher_reader = Arc::clone(&aes_cipher);
    let aes_cipher_writer = Arc::clone(&aes_cipher);


    // Thread to read from server and print to stdout
    thread::spawn(move || {
        let cipher = aes_cipher_reader;
        let mut reader = BufReader::new(read_stream);
    
        loop {
            // 1. Read nonce (12 bytes)
            let mut nonce_bytes = [0u8; 12];
            if let Err(e) = reader.read_exact(&mut nonce_bytes) {
                eprintln!("Error reading nonce: {}", e);
                break;
            }
            let nonce = Nonce::from_slice(&nonce_bytes);
    
            // 2. Read encrypted message size (2 bytes)
            let mut size_buf = [0u8; 2];
            if let Err(e) = reader.read_exact(&mut size_buf) {
                eprintln!("Error reading size: {}", e);
                break;
            }
            let size = u16::from_be_bytes(size_buf) as usize;
    
            // 3. Read encrypted message
            let mut encrypted_msg = vec![0u8; size];
            if let Err(e) = reader.read_exact(&mut encrypted_msg) {
                eprintln!("Error reading encrypted message: {}", e);
                break;
            }
    
            // 4. Decrypt
            match cipher.decrypt(nonce, encrypted_msg.as_ref()) {
                Ok(plaintext) => {
                    if let Ok(text) = String::from_utf8(plaintext) {
                        println!("{}", text); // output for GTK
                    } else {
                        eprintln!("Decrypted message not valid UTF-8");
                    }
                }
                Err(_) => {
                    eprintln!("Failed to decrypt message");
                }
            }
        }
    });
    

    // Thread to write messages to server
    thread::spawn(move || {
        let cipher = aes_cipher_writer;
        let mut write_stream = write_stream;

        for full_msg in rx {
            // 1. Generate random nonce
            let mut nonce_bytes = [0u8; 12];
            rand::thread_rng().fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from_slice(&nonce_bytes);

            // 2. Encrypt message
            match cipher.encrypt(nonce, full_msg.as_bytes()) {
                Ok(ciphertext) => {
                    // 3. Write: nonce + size + ciphertext
                    let size = ciphertext.len() as u16;
                    let size_bytes = size.to_be_bytes();

                    let mut packet = Vec::with_capacity(12 + 2 + ciphertext.len());
                    packet.extend_from_slice(&nonce_bytes);
                    packet.extend_from_slice(&size_bytes);
                    packet.extend_from_slice(&ciphertext);

                    if write_stream.write_all(&packet).is_err() {
                        break;
                    }

                }
                Err(_) => {
                    eprintln!("Encryption failed!");
                }
            }
        }
    });

    for line in stdin_reader.lines() {
        let msg = line?;
        let full_msg = format!("{}: {}", username, msg);
        if tx.send(full_msg).is_err() {
            break;
        }
    }



    Ok(())
}

// thread::spawn(move || {
    //     for msg in rx {
    //         writeln!(&write_stream, "{}", msg).ok();
    //     }
    // });

    // Main thread reads messages from GTK (via stdin) and sends them to server
    // for line in stdin_reader.lines() {
    //     let msg = line?;
    //     if tx.send(msg).is_err() {
    //         break;
    //     }
    // }