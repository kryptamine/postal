//! An SMTP server inside the test process, in plaintext on `127.0.0.1:0`.
//! It takes `AUTH PLAIN` with the app password and `AUTH XOAUTH2` with the
//! access token, and records each sign-in and each Message it accepts.

use std::{
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use base64::{Engine, engine::general_purpose::STANDARD};

use super::fake_imap::{EMAIL, PASSWORD, xoauth2};

/// One Message as the server took it.
#[derive(Clone, Debug)]
pub struct Received {
    pub mail_from: String,
    pub rcpt_to: Vec<String>,
    /// The Message after `DATA`, its dots unstuffed.
    pub data: String,
}

#[derive(Default)]
struct State {
    /// Each `AUTH`'s mechanism and decoded response.
    logins: Vec<(String, String)>,
    received: Vec<Received>,
    /// The answer to the end of the data, in place of `250`.
    refusal: Option<String>,
    /// Each connection is closed before the greeting.
    down: bool,
    /// Connections closed so, as each try to send makes one.
    turned_away: usize,
}

pub struct FakeSmtp {
    pub port: u16,
    state: Arc<Mutex<State>>,
}

impl FakeSmtp {
    pub fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(Mutex::new(State::default()));
        let served = Arc::clone(&state);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let mut state = served.lock().unwrap();
                if state.down {
                    state.turned_away += 1;
                    continue;
                }

                drop(state);

                let served = Arc::clone(&served);
                thread::spawn(move || serve(stream, &served));
            }
        });

        Self { port, state }
    }

    pub fn logins(&self) -> Vec<(String, String)> {
        self.state.lock().unwrap().logins.clone()
    }

    pub fn received(&self) -> Vec<Received> {
        self.state.lock().unwrap().received.clone()
    }

    /// Answers the end of every Message's data with `reply` from now on,
    /// such as `554 5.7.1 rejected`; nothing is recorded as received.
    pub fn refuse(&self, reply: &str) {
        self.state.lock().unwrap().refusal = Some(reply.to_owned());
    }

    /// Closes every connection at once from now on, as a server down would.
    pub fn down(&self) {
        self.state.lock().unwrap().down = true;
    }

    pub fn turned_away(&self) -> usize {
        self.state.lock().unwrap().turned_away
    }

    pub fn up(&self) {
        self.state.lock().unwrap().down = false;
    }
}

fn serve(stream: TcpStream, state: &Mutex<State>) {
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let mut say = |reply: &str| writer.write_all(format!("{reply}\r\n").as_bytes());
    if say("220 fake ESMTP").is_err() {
        return;
    }

    let (mut mail_from, mut rcpt_to) = (String::new(), Vec::new());
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
        let command = line.trim_end().to_owned();
        line.clear();
        let verb = command
            .split_once([' ', ':'])
            .map_or(command.as_str(), |(verb, _)| verb)
            .to_ascii_uppercase();
        let reply = match verb.as_str() {
            "EHLO" => "250-fake\r\n250-AUTH PLAIN XOAUTH2\r\n250 8BITMIME".to_owned(),
            "AUTH" => auth(&command, state),
            "MAIL" => {
                mail_from = address(&command);
                rcpt_to.clear();
                "250 2.1.0 OK".to_owned()
            }
            "RCPT" => {
                rcpt_to.push(address(&command));
                "250 2.1.5 OK".to_owned()
            }
            "DATA" => {
                if say("354 Go ahead").is_err() {
                    return;
                }

                let Some(data) = data(&mut reader) else {
                    return;
                };

                let received = Received {
                    mail_from: mail_from.clone(),
                    rcpt_to: rcpt_to.clone(),
                    data,
                };

                take(received, state)
            }
            "RSET" | "NOOP" => "250 2.0.0 OK".to_owned(),
            "QUIT" => {
                say("221 2.0.0 Bye").ok();
                return;
            }
            _ => "502 5.5.1 Unrecognized command".to_owned(),
        };

        if say(&reply).is_err() {
            return;
        }
    }
}

/// Records `received`, unless the server refuses every Message now.
fn take(received: Received, state: &Mutex<State>) -> String {
    let mut state = state.lock().unwrap();
    if let Some(refusal) = &state.refusal {
        return refusal.clone();
    }

    state.received.push(received);
    drop(state);
    "250 2.0.0 OK queued".to_owned()
}

/// Checks `AUTH <mechanism> <initial response>` against the Account's
/// secrets, as Gmail would.
fn auth(command: &str, state: &Mutex<State>) -> String {
    let mut words = command.split(' ').skip(1);
    let mechanism = words.next().unwrap_or_default().to_owned();
    let response = words
        .next()
        .and_then(|encoded| STANDARD.decode(encoded).ok())
        .map(|decoded| String::from_utf8(decoded).unwrap())
        .unwrap_or_default();
    let ok = match mechanism.as_str() {
        "PLAIN" => response == format!("\0{EMAIL}\0{PASSWORD}"),
        "XOAUTH2" => response == xoauth2(),
        _ => false,
    };

    state.lock().unwrap().logins.push((mechanism, response));
    if ok {
        "235 2.7.0 Accepted".to_owned()
    } else {
        "535 5.7.8 Username and Password not accepted".to_owned()
    }
}

/// The address inside `MAIL FROM:<...>` or `RCPT TO:<...>`.
fn address(command: &str) -> String {
    let start = command.find('<').map_or(0, |at| at + 1);
    let end = command.rfind('>').unwrap_or(command.len());
    command.get(start..end).unwrap_or_default().to_owned()
}

/// The lines up to the lone `.`, with CRLF kept and dots unstuffed.
fn data(reader: &mut impl BufRead) -> Option<String> {
    let mut data = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }

        if line == ".\r\n" {
            return Some(data);
        }

        data.push_str(line.strip_prefix('.').unwrap_or(&line));
    }
}
