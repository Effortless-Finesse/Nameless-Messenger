use std::{
    io::{self, BufRead, BufReader, Write, Read},
    net::TcpStream,
    sync::mpsc,
    thread,
};

fn main() -> io::Result<()> {
    // Ask for username
    print!("Choose your username: ");
    io::stdout().flush()?;
    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    let username = username.trim().to_string();

    // Connect to the lobby
    let mut lobby_stream = TcpStream::connect("localhost:8080")?;
    write!(lobby_stream, "client {}\n", username)?;
    lobby_stream.flush()?; // Ensure data is sent

    let mut reader = BufReader::new(lobby_stream.try_clone()?);

    // Read list of servers
    println!("\nAvailable Servers:");
    for line in reader.by_ref().lines() {
        let line = line?;
        if line.trim().is_empty() {
            break;
        }
        println!("{}", line);
    }

    // Ask user which server to connect to
    print!("\nEnter index number of server to connect to: ");
    io::stdout().flush()?;
    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();

    // Tell the lobby which server you chose
    writeln!(lobby_stream, "{}\n", choice)?;
    lobby_stream.flush()?;


    // Connect directly to the chosen server
    let mut buffer = [0u8; 128];
    let size = lobby_stream.read(&mut buffer)?;
    let target_ip = String::from_utf8_lossy(&buffer[..size]).trim().to_string();

    println!("Connecting to server at: {}", target_ip);
    let server_stream = TcpStream::connect(target_ip)?;

    write!(&server_stream, "client {}\n", username);

    let read_stream = server_stream.try_clone()?;
    let write_stream = server_stream.try_clone()?;

    // Channel for user input
    let (tx, rx) = mpsc::channel::<String>();

    // Thread to read from server
    thread::spawn(move || {
        let reader = BufReader::new(read_stream);
        for line in reader.lines() {
            match line {
                Ok(msg) => println!("{}", msg),
                  Err(e) => {
                      eprintln!("Error reading from server: {}", e);
                      break;
                  }
            }
        }
    });

    // Thread to write to server
    thread::spawn(move || {
        for msg in rx {
            writeln!(&write_stream, "{}", msg).ok();
        }
    });

    // Main thread handles user input
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let msg = line?;
        if tx.send(msg).is_err() {
            break;
        }
    }

    Ok(())
}
