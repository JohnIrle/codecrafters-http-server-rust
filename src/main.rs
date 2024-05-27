use concurrency::ThreadPool;
use std::env;
use std::io::{BufRead, Read, Write};
use std::net::TcpListener;

mod concurrency;
mod handler;
mod parsing;

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
                    handler::handle_connection(stream, &directory).unwrap();
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
