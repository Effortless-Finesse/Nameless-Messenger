use std::{
    io::{self, BufRead, BufReader, Write, Read},
    net::TcpStream,
    sync::{mpsc, Arc, Mutex},
    thread,
};

// Crypto imports
use rsa::{RsaPublicKey, RsaPrivateKey, Pkcs1v15Encrypt};
use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit, generic_array::GenericArray},
};
use rand::rngs::OsRng;
use rand::RngCore;
use hex;
use rsa::pkcs1::{EncodeRsaPublicKey, DecodeRsaPublicKey, LineEnding};

struct ClientKeys {
    priv_key: RsaPrivateKey,
    pub_key: RsaPublicKey,
    aes_key: Option<Vec<u8>>,
}

impl ClientKeys {
    fn new() -> Self {
        let mut rng = OsRng;
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub_key = RsaPublicKey::from(&priv_key);
        ClientKeys { priv_key, pub_key, aes_key: None }
    }
}

fn main() -> io::Result<()> {
    // Read username
    eprintln!("Started");
    let mut stdin_reader = BufReader::new(io::stdin());
    let mut username = String::new();
    stdin_reader.read_line(&mut username)?;
    let username = username.trim().to_string();
    eprintln!("Username received: '{}'", username);

    // Initialize crypto
    let keys = Arc::new(Mutex::new(ClientKeys::new()));
    let keys_clone = Arc::clone(&keys);

    // Connect to lobby
    let mut lobby_stream = TcpStream::connect("localhost:8080")?;
    write!(lobby_stream, "client {}\n", username)?;
    lobby_stream.flush()?;

    // Get server IP
    let mut buffer = [0u8; 128];
    let size = lobby_stream.read(&mut buffer)?;
    let target_ip = String::from_utf8_lossy(&buffer[..size]).trim().to_string();

    // Connect to server
    let mut server_stream = TcpStream::connect(target_ip)?;
    write!(server_stream, "client {}\n", username)?;
    server_stream.flush()?;

    // Send public key
    let pub_key_pem = keys.lock().unwrap().pub_key.to_pkcs1_pem(LineEnding::LF).unwrap();
    writeln!(server_stream, "PUBKEY {}", pub_key_pem)?;

    // Clone stream for separate read/write
    let read_stream = server_stream.try_clone()?;
    let write_stream = server_stream.try_clone()?;

    // Channel for UI messages
    let (tx, rx) = mpsc::channel::<String>();

    // Thread 1: Handle incoming messages
    let keys_msg = Arc::clone(&keys);
    thread::spawn(move || {
        let mut reader = BufReader::new(read_stream);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() { break; }

            if line.starts_with("AESKEY ") {
                if let Ok(encrypted_key) = hex::decode(line[7..].trim()) {
                    let decrypted = keys_msg.lock().unwrap().priv_key
                        .decrypt(Pkcs1v15Encrypt, &encrypted_key).unwrap();
                    keys_msg.lock().unwrap().aes_key = Some(decrypted);
                }
            } else if line.starts_with("ENC ") {
                let parts: Vec<&str> = line[4..].trim().split_whitespace().collect();
                if parts.len() == 2 {
                    if let (Ok(nonce), Ok(ciphertext)) = (
                        hex::decode(parts[0]),
                        hex::decode(parts[1])
                    ) {
                        if let Some(aes_key) = &keys_msg.lock().unwrap().aes_key {
                            let cipher = Aes256Gcm::new(GenericArray::from_slice(aes_key));
                            if let Ok(plaintext) = cipher.decrypt(
                                GenericArray::from_slice(&nonce),
                                ciphertext.as_ref()  // FIXED: Using as_ref() instead of &
                            ) {
                                println!("{}", String::from_utf8_lossy(&plaintext));
                                continue;
                            }
                        }
                    }
                }
            }
            println!("{}", line.trim());
        }
    });

    // Thread 2: Send outgoing messages
    thread::spawn(move || {
        for msg in rx {
            if let Err(e) = writeln!(&write_stream, "{}", msg) {
                eprintln!("Failed to send message: {}", e);
                break;
            }
        }
    });

    // Main thread: Read user input and encrypt
    for line in stdin_reader.lines() {
        let msg = line?;
        let keys = keys.lock().unwrap();
        
        if let Some(aes_key) = &keys.aes_key {
            // AES-GCM encryption
            let cipher = Aes256Gcm::new(GenericArray::from_slice(aes_key));
            let mut nonce = [0u8; 12];
            rand::thread_rng().fill_bytes(&mut nonce);
            
            match cipher.encrypt(
                &GenericArray::from_slice(&nonce),
                msg.as_bytes()
            ) {
                Ok(encrypted) => {
                    tx.send(format!(
                        "ENC {} {}",
                        hex::encode(nonce),
                        hex::encode(encrypted)
                    )).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                }
                Err(e) => eprintln!("Encryption failed: {}", e),
            }
        } else {
            // Unencrypted fallback
            tx.send(format!("{}: {}", username, msg))
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    }

    Ok(())
}