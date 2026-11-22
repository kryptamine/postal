//! The domain's own autoconfig and Thunderbird's ISPDB inside the test
//! process, over plain HTTP on `127.0.0.1:0`: a document per path, 404
//! for the rest.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{Arc, Mutex, OnceLock},
    thread,
};

use super::fake_dns::FakeDns;

#[derive(Default)]
struct Served {
    documents: HashMap<String, String>,
    paths: Vec<String>,
}

pub struct FakeAutoconfig {
    pub port: u16,
    served: Arc<Mutex<Served>>,
}

/// One autoconfig and one DNS that publish nothing, for a test that only
/// needs the guess from the domain.
pub fn nothing_published() -> String {
    static NOTHING: OnceLock<(FakeAutoconfig, FakeDns)> = OnceLock::new();
    let (http, dns) = NOTHING.get_or_init(|| (FakeAutoconfig::start(), FakeDns::start()));
    http.config(dns)
}

impl FakeAutoconfig {
    pub fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let served = Arc::new(Mutex::new(Served::default()));
        let serving = Arc::clone(&served);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                answer(stream, &serving);
            }
        });

        Self { port, served }
    }

    /// The `[autoconfig]` table that points Postal at this and `dns`.
    pub fn config(&self, dns: &FakeDns) -> String {
        format!(
            "\n[autoconfig]\ndomain-url = \"http://127.0.0.1:{0}/own/{{domain}}\"\n\
             ispdb-url = \"http://127.0.0.1:{0}/ispdb/{{domain}}\"\ndns-server = \"127.0.0.1:{1}\"\n",
            self.port, dns.port
        )
    }

    /// Serves `xml` as the domain's own document for `domain`.
    pub fn publish_own(&self, domain: &str, xml: &str) {
        self.publish(&format!("/own/{domain}"), xml);
    }

    pub fn publish_ispdb(&self, domain: &str, xml: &str) {
        self.publish(&format!("/ispdb/{domain}"), xml);
    }

    fn publish(&self, path: &str, xml: &str) {
        let mut served = self.served.lock().unwrap();
        served.documents.insert(path.to_owned(), xml.to_owned());
    }

    /// The path of each request, in order.
    pub fn paths(&self) -> Vec<String> {
        self.served.lock().unwrap().paths.clone()
    }
}

fn answer(stream: TcpStream, served: &Mutex<Served>) {
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
    let document = {
        let mut served = served.lock().unwrap();
        served.paths.push(path.clone());
        served.documents.get(&path).cloned()
    };

    let (status, body) =
        document.map_or_else(|| ("404 Not Found", String::new()), |xml| ("200 OK", xml));

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    reader.into_inner().write_all(response.as_bytes()).ok();
}
