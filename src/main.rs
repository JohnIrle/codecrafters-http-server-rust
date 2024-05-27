use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::{env, fs, io, thread};

struct Worker {
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => job(),
                Err(_) => break,
            }
        });

        Self {
            thread: Some(thread),
        }
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "ThreadPool size must be greater than 0");

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for _ in 0..size {
            workers.push(Worker::new(Arc::clone(&receiver)));
        }

        Self {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                #[allow(clippy::expect_used)]
                thread.join().expect("failed to join");
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let dir = if args.len() > 2 {
        args.windows(2)
            .find(|window| window[0] == "--directory")
            .unwrap_or_default()[1]
            .to_owned()
    } else {
        String::new()
    };

    let listener = TcpListener::bind("127.0.0.1:4221").expect("Could not establish TcpListener");
    let pool = ThreadPool::new(4);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let directory = dir.clone();
                pool.execute(move || {
                    println!("accepted new connection");
                    handle_connection(stream, &directory).unwrap();
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

const ACCEPTED_ENCODING: [&str; 1] = ["gzip"];

fn handle_connection(mut stream: TcpStream, directory: &str) -> io::Result<()> {
    let mut buf_reader = BufReader::new(&mut stream);

    let Ok((method, path, headers)) = parse_request(&mut buf_reader) else {
        stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")?;
        return Ok(());
    };

    let parsed_headers: Vec<_> = headers.split("\r\n").collect();

    match (method.as_str(), path.as_str()) {
        ("GET", "/") => stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n")?,
        ("GET", path) if path.starts_with("/echo") => {
            let message = path.trim_start_matches("/echo/").to_owned();
            let content_encoding_header = match parsed_headers
                .iter()
                .find_map(|line| line.strip_prefix("Accept-Encoding:").map(str::trim))
            {
                Some(encoding) if ACCEPTED_ENCODING.contains(&encoding) => {
                    format!("Content-Encoding: {}\r\n", encoding)
                }
                _ => String::new(),
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\n{}Content-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                content_encoding_header,
                message.len(),
                message
            );
            stream.write_all(response.as_bytes())?;
        }
        ("GET", path) if path.starts_with("/user-agent") => {
            let Some(user_agent) = parsed_headers
                .iter()
                .find_map(|line| line.strip_prefix("User-Agent:").map(str::trim))
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

            let Some(body) = parse_body(&mut buf_reader, &headers) else {
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

fn parse_request(
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

    Ok((method, route, headers))
}

fn parse_body(reader: &mut BufReader<&mut TcpStream>, headers: &str) -> Option<String> {
    let content_length = headers
        .lines()
        .find(|&line| line.starts_with("Content-Length:"))
        .and_then(|line| line.split(": ").nth(1))
        .and_then(|len| len.trim().parse::<usize>().ok())
        .unwrap_or(0);

    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).ok()?;
    Some(String::from_utf8_lossy(&body).to_string())
}
