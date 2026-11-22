//! A DNS server inside the test process, over UDP on `127.0.0.1:0`,
//! answering SRV queries from the records it is given; NXDOMAIN for the rest.

use std::{
    collections::{HashMap, HashSet},
    net::{Ipv4Addr, UdpSocket},
    sync::{Arc, Mutex},
    thread,
};

/// One SRV record: priority, weight, port and target.
pub type Srv = (u16, u16, u16, &'static str);

#[derive(Default)]
struct Zone {
    records: HashMap<String, Vec<Srv>>,
    /// Names whose queries are never answered.
    silent: HashSet<String>,
    asked: Vec<String>,
}

pub struct FakeDns {
    pub port: u16,
    zone: Arc<Mutex<Zone>>,
}

impl FakeDns {
    pub fn start() -> Self {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        let zone = Arc::new(Mutex::new(Zone::default()));
        let serving = Arc::clone(&zone);
        thread::spawn(move || {
            let mut buffer = [0; 512];
            while let Ok((length, from)) = socket.recv_from(&mut buffer) {
                if let Some(response) = answer(&buffer[..length], &serving) {
                    socket.send_to(&response, from).ok();
                }
            }
        });

        Self { port, zone }
    }

    pub fn add(&self, name: &str, records: &[Srv]) {
        let mut zone = self.zone.lock().unwrap();
        zone.records.insert(name.to_owned(), records.to_vec());
    }

    pub fn silence(&self, name: &str) {
        self.zone.lock().unwrap().silent.insert(name.to_owned());
    }

    /// Each name asked, in order.
    pub fn asked(&self) -> Vec<String> {
        self.zone.lock().unwrap().asked.clone()
    }
}

fn answer(query: &[u8], zone: &Mutex<Zone>) -> Option<Vec<u8>> {
    let mut at = 12;
    let mut labels = Vec::new();
    while query[at] != 0 {
        let length = usize::from(query[at]);
        labels.push(String::from_utf8_lossy(&query[at + 1..=at + length]).into_owned());
        at += 1 + length;
    }

    let question_end = at + 5;
    let name = labels.join(".");
    let records = {
        let mut zone = zone.lock().unwrap();
        zone.asked.push(name.clone());
        if zone.silent.contains(&name) {
            return None;
        }

        zone.records.get(&name).cloned()
    };

    let mut response = query[..2].to_vec();
    let code = if records.is_some() { 0x80 } else { 0x83 };
    response.extend([0x81, code, 0, 1]);
    let records = records.unwrap_or_default();
    response.extend(u16::try_from(records.len()).unwrap().to_be_bytes());
    response.extend([0, 0, 0, 0]);
    response.extend(&query[12..question_end]);
    for (priority, weight, port, target) in records {
        let mut data = Vec::new();
        for value in [priority, weight, port] {
            data.extend(value.to_be_bytes());
        }

        for label in target.split('.').filter(|label| !label.is_empty()) {
            data.push(u8::try_from(label.len()).unwrap());
            data.extend(label.as_bytes());
        }

        data.push(0);
        // The owner is a pointer to the question's name.
        response.extend([0xc0, 12, 0, 33, 0, 1, 0, 0, 1, 44]);
        response.extend(u16::try_from(data.len()).unwrap().to_be_bytes());
        response.extend(data);
    }

    Some(response)
}
