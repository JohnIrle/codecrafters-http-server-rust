use std::io::{BufRead, BufReader, Write};
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
    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();

    let request = match http_request.first() {
        Some(line) => line,
        _ => {
            stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")?;
            println!("request");
            return Ok(());
        }
    };

    let parts: Vec<&str> = request.split_whitespace().collect();
    let path = match parts.get(1) {
        Some(&path) => path,
        None => {
            stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")?;
            println!("path");
            return Ok(());
        }
    };

    match path {
        "/" => stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n")?,
        path if path.starts_with("/echo") => {
            let message = path.trim_start_matches("/echo/").to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                message.len(),
                message
            );
            stream.write_all(response.as_bytes())?;
        }
        path if path.starts_with("/user-agent") => {
            let user_agent = match http_request.iter().find(|line| line.starts_with("User")) {
                Some(line) => line,
                _ => {
                    stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")?;
                    println!("useragent");
                    return Ok(());
                }
            };
            let trimmed_user_agent = user_agent.trim_start_matches("User-Agent:").trim();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                trimmed_user_agent.len(),
                trimmed_user_agent
            );
            stream.write_all(response.as_bytes())?;
        }
        _ => {
            println!("last branch");
            stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n")?
        }
    }

    Ok(())
}
