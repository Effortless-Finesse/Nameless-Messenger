use std::{
    io::{self, BufRead, BufReader, Write},
    net::TcpStream,
    sync::mpsc,
    thread,
};

fn main() -> io::Result<()> {
    // Connect to the server
    let stream = TcpStream::connect("localhost:8080").expect("Could not connect to server");

    // Clone the stream for reading
    let read_stream = stream.try_clone().expect("Could not clone stream");

    // Channel for sending messages from user input to the write thread
    let (tx, rx) = mpsc::channel::<String>();

    // Thread to read messages from server
    thread::spawn(move || {
        let reader = BufReader::new(read_stream);
        for line in reader.lines() {
            match line {
                Ok(msg) => println!("Server: {}", msg),
                  Err(e) => {
                      eprintln!("Error reading from server: {}", e);
                      break;
                  }
            }
        }
    });

    // Thread to write messages to the server
    let write_stream = stream.try_clone().expect("Could not clone stream for writing");
    thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            writeln!(&write_stream, "{}", msg).unwrap();
        }
    });

    // Main thread handles user input
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let msg = line.unwrap();
        if tx.send(msg).is_err() {
            break; // Stop if the receiver has disconnected
        }
    }

    Ok(())
}
