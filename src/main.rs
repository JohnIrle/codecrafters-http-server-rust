use std::io::{BufRead, BufReader, Write};
// Uncomment this block to pass the first stage
use std::net::{TcpListener, TcpStream};

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    // Uncomment this block to pass the first stage

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                handle_connection(&mut stream).unwrap();
                println!("accepted new connection");
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

fn handle_connection(mut stream: &mut TcpStream) -> std::io::Result<()> {
    let buf_reader = BufReader::new(&mut stream);

    if let Some(Ok(request_line)) = buf_reader.lines().next() {
        let parts: Vec<&str> = request_line.split(' ').collect();

        if let Some(path) = parts.get(1) {
            match *path {
                "/" => stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n")?,
                path if path.starts_with("/echo") => {
                    let message = path.trim_start_matches("/echo").trim_start_matches('/');
                    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}", message.len(), message).as_bytes())?;
                }
                _ => stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n")?,
            }
        } else {
            stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")?
        }
    } else {
        stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")?
    }

    Ok(())
}
