use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;

pub fn parse_request(
    reader: &mut BufReader<&mut TcpStream>,
) -> Result<(String, String, String), &'static str> {
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|_| "Failed to read request line")?;
    let parts: Vec<&str> = request_line.split_whitespace().collect();

    if parts.len() != 3 {
        return Err("Invalid request line");
    }

    let method = parts[0].to_owned();
    let route = parts[1].to_owned();

    let mut headers = String::new();
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|_| "Failed to read headers")?;
        if line == "\r\n" {
            break;
        }
        headers.push_str(&line);
    }

    Ok((method, route, headers.to_lowercase()))
}

pub fn parse_body(reader: &mut BufReader<&mut TcpStream>, headers: &str) -> Option<String> {
    let content_length = headers
        .lines()
        .find(|&line| line.starts_with("content-length:"))
        .and_then(|line| line.split(": ").nth(1))
        .and_then(|len| len.trim().parse::<usize>().ok())
        .unwrap_or(0);

    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).ok()?;
    Some(String::from_utf8_lossy(&body).to_string())
}
