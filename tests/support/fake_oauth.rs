//! Google's token endpoint inside the test process, over plain HTTP on
//! `127.0.0.1:0`.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use super::{
    fake_imap::{ACCESS_TOKEN, EMAIL},
    query::{self, Form},
};

/// The authorization code the consent hands back.
pub const CODE: &str = "4/0AfakeAuthorizationCode";
pub const REFRESH_TOKEN: &str = "1//fakeRefreshToken";
const MAIL_SCOPE: &str = "https://mail.google.com/";

struct Grants {
    scope: String,
    /// Seconds, as each access token is granted.
    expires_in: u64,
    /// The refresh token is refused with `invalid_grant`.
    revoked: bool,
    requests: Vec<Form>,
}

pub struct FakeOAuth {
    pub port: u16,
    grants: Arc<Mutex<Grants>>,
}

impl FakeOAuth {
    pub fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let grants = Arc::new(Mutex::new(Grants {
            scope: format!("{MAIL_SCOPE} openid https://www.googleapis.com/auth/userinfo.email"),
            expires_in: 3599,
            revoked: false,
            requests: Vec::new(),
        }));
        let served = Arc::clone(&grants);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                answer(stream, &served);
            }
        });

        Self { port, grants }
    }

    pub fn token_url(&self) -> String {
        format!("http://127.0.0.1:{}/token", self.port)
    }

    /// The scopes the next grants say the user agreed to.
    pub fn grant_scope(&self, scope: &str) {
        scope.clone_into(&mut self.grants.lock().unwrap().scope);
    }

    pub fn expire_in(&self, seconds: u64) {
        self.grants.lock().unwrap().expires_in = seconds;
    }

    pub fn revoke(&self) {
        self.grants.lock().unwrap().revoked = true;
    }

    /// Each form posted to the token endpoint, in order.
    pub fn requests(&self) -> Vec<Form> {
        self.grants.lock().unwrap().requests.clone()
    }
}

fn answer(stream: TcpStream, grants: &Mutex<Grants>) {
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }

        let line = line.trim_end();
        if line.is_empty() {
            break;
        }

        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap();
        }
    }

    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    let form = query::parse(std::str::from_utf8(&body).unwrap());
    let (status, json) = grant(&form, grants);
    grants.lock().unwrap().requests.push(form);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    );
    let _ = reader.into_inner().write_all(response.as_bytes());
}

fn grant(form: &Form, grants: &Mutex<Grants>) -> (&'static str, String) {
    let (scope, expires_in, revoked) = {
        let grants = grants.lock().unwrap();
        (grants.scope.clone(), grants.expires_in, grants.revoked)
    };

    let field = |name: &str| form.get(name).map(String::as_str);
    match field("grant_type") {
        Some("authorization_code") if field("code") == Some(CODE) => {
            let id_token = id_token(EMAIL);
            (
                "200 OK",
                format!(
                    r#"{{"access_token":"{ACCESS_TOKEN}","expires_in":{expires_in},"refresh_token":"{REFRESH_TOKEN}","scope":"{scope}","token_type":"Bearer","id_token":"{id_token}"}}"#
                ),
            )
        }
        Some("refresh_token") if !revoked && field("refresh_token") == Some(REFRESH_TOKEN) => (
            "200 OK",
            format!(
                r#"{{"access_token":"{ACCESS_TOKEN}","expires_in":{expires_in},"scope":"{scope}","token_type":"Bearer"}}"#
            ),
        ),
        _ => (
            "400 Bad Request",
            r#"{"error":"invalid_grant","error_description":"Bad Request"}"#.to_owned(),
        ),
    }
}

/// An unsigned JWT: Postal takes it straight from the token endpoint over
/// TLS, so it reads the claims without checking a signature.
fn id_token(email: &str) -> String {
    let part = |json: &str| URL_SAFE_NO_PAD.encode(json);
    format!(
        "{}.{}.{}",
        part(r#"{"alg":"RS256","typ":"JWT"}"#),
        part(&format!(r#"{{"email":"{email}","email_verified":true}}"#)),
        part("signature")
    )
}
