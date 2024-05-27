use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::{fs, io};

const ACCEPTED_ENCODING: [&str; 1] = ["gzip"];

pub fn handle_connection(mut stream: TcpStream, directory: &str) -> io::Result<()> {
    let mut buf_reader = BufReader::new(&mut stream);

    let Ok((method, path, headers)) = crate::parsing::parse_request(&mut buf_reader) else {
        stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")?;
        return Ok(());
    };

    let parsed_headers: Vec<_> = headers.split("\r\n").collect();

    match (method.as_str(), path.as_str()) {
        ("GET", "/") => stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n")?,
        ("GET", path) if path.starts_with("/echo") => {
            let message = path.trim_start_matches("/echo/").to_owned();
            // TODO: accept more than gzip
            if let Some(encoding) = parsed_headers
                .iter()
                .find_map(|line| line.strip_prefix("accept-encoding:"))
                .and_then(|encodings| {
                    encodings
                        .split(',')
                        .map(str::trim)
                        .find(|encoding| ACCEPTED_ENCODING.contains(encoding))
                })
            {
                let mut response = Vec::new();

                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(message.as_bytes())?;
                let compressed_message = encoder.finish()?;
                write!(
                    response,
                    "HTTP/1.1 200 OK\r\nContent-Encoding: {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n",
                    encoding,
                    compressed_message.len()
                )?;
                response.extend_from_slice(&compressed_message);
                stream.write_all(&response)?;
            } else {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    message.len(),
                    message
                );
                stream.write_all(response.as_bytes())?;
            };
        }
        ("GET", path) if path.starts_with("/user-agent") => {
            let Some(user_agent) = parsed_headers
                .iter()
                .find_map(|line| line.strip_prefix("user-agent:").map(str::trim))
            else {
                stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")?;
                return Ok(());
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                user_agent.len(),
                user_agent
            );
            stream.write_all(response.as_bytes())?;
        }
        ("GET", path) if path.starts_with("/files/") => {
            let file_name = path.replace("/files/", "");
            let file_path = format!("{directory}/{file_name}");

            let contents = fs::read_to_string(file_path);

            let Ok(file) = contents else {
                stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n")?;
                return Ok(());
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n{}",
                file.len(),
                file
            );
            stream.write_all(response.as_bytes())?;
        }
        ("POST", path) if path.starts_with("/files/") => {
            let file_name = path.replace("/files/", "");

            let file_path = directory.to_owned() + &file_name;

            let Ok(mut file) = File::create(file_path) else {
                stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\n\r\n")?;
                return Ok(());
            };

            let Some(body) = crate::parsing::parse_body(&mut buf_reader, &headers) else {
                stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")?;
                return Ok(());
            };

            if matches!(file.write_all(body.as_bytes()), Ok(())) {
                stream.write_all(b"HTTP/1.1 201 Created\r\n\r\n")?;
            } else {
                stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\n\r\n")?;
                return Ok(());
            }
        }
        _ => stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n")?,
    }

    Ok(())
}
