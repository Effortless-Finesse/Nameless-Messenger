use std::{
    io::{self, BufRead, BufReader, Write, Read},
    net::TcpStream,
    sync::mpsc,
    thread,
};

fn main() -> io::Result<()> {
    // Read the username sent from the GTK UI via stdin
    eprintln!("Started");
    let mut stdin_reader = BufReader::new(io::stdin());
    let mut username = String::new();
    stdin_reader.read_line(&mut username)?;
    let username = username.trim().to_string();
    eprintln!("Username received: '{}'", username);


    // Connect to the lobby
    let mut lobby_stream = TcpStream::connect("localhost:8080")?;
    write!(lobby_stream, "client {}\n", username)?;
    lobby_stream.flush()?; // Ensure data is sent

    // Read server IP from lobby
    let mut buffer = [0u8; 128];
    let size = lobby_stream.read(&mut buffer)?;
    let target_ip = String::from_utf8_lossy(&buffer[..size]).trim().to_string();

    // Connect directly to the chosen server
    let mut server_stream = TcpStream::connect(target_ip)?;
    write!(server_stream, "client {}\n", username)?;
    server_stream.flush()?;

    let read_stream = server_stream.try_clone()?;
    let write_stream = server_stream.try_clone()?;

    // Channel for UI input
    let (tx, rx) = mpsc::channel::<String>();

    // Thread to read from server and print to stdout
    thread::spawn(move || {
        let reader = BufReader::new(read_stream);
        for line in reader.lines() {
            match line {
                Ok(msg) => println!("{}", msg), // Output to stdout, picked up by C
                Err(e) => {
                    eprintln!("Error reading from server: {}", e);
                    break;
                }
            }
        }
    });

    // Thread to write messages to server
    thread::spawn(move || {
        for msg in rx {
            writeln!(&write_stream, "{}", msg).ok();
        }
    });

    // Main thread reads messages from GTK (via stdin) and sends them to server
    for line in stdin_reader.lines() {
        let msg = line?;
        if tx.send(msg).is_err() {
            break;
        }
    }

    Ok(())
}
