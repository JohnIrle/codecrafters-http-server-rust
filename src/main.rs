use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");

                    job();
                }
                Err(_) => {
                    break;
                }
            }
        });

        Worker {
            id,
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
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
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
                thread.join().unwrap();
            }
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    let pool = ThreadPool::new(4);

    for stream in listener.incoming().take(2) {
        match stream {
            Ok(mut stream) => {
                pool.execute(move || {
                    println!("accepted new connection");
                    handle_connection(&mut stream).unwrap();
                });
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
