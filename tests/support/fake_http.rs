//! A plain HTTP server inside the test process on `127.0.0.1:0`, answering
//! each request's path through a route and recording the paths in order.

use std::{
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

pub enum Reply {
    Body(Vec<u8>),
    Redirect(String),
    NotFound,
}

pub struct FakeHttp {
    pub port: u16,
    paths: Arc<Mutex<Vec<String>>>,
}

impl FakeHttp {
    pub fn start(route: impl Fn(&str) -> Reply + Send + 'static) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let paths = Arc::new(Mutex::new(Vec::new()));
        let recording = Arc::clone(&paths);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                answer(stream, &route, &recording);
            }
        });

        Self { port, paths }
    }

    pub fn paths(&self) -> Vec<String> {
        self.paths.lock().unwrap().clone()
    }
}

fn answer(stream: TcpStream, route: &impl Fn(&str) -> Reply, paths: &Mutex<Vec<String>>) {
    let mut reader = BufReader::new(stream);
    let mut request = String::new();
    if reader.read_line(&mut request).unwrap_or(0) == 0 {
        return;
    }

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim_end().is_empty() {
            break;
        }
    }

    let path = request.split(' ').nth(1).unwrap_or_default().to_owned();
    paths.lock().unwrap().push(path.clone());
    let (head, body) = match route(&path) {
        Reply::Body(body) => ("200 OK".to_owned(), body),
        Reply::Redirect(location) => (format!("302 Found\r\nLocation: {location}"), Vec::new()),
        Reply::NotFound => ("404 Not Found".to_owned(), Vec::new()),
    };

    let mut stream = reader.into_inner();
    let head = format!(
        "HTTP/1.1 {head}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).ok();
    stream.write_all(&body).ok();
}
