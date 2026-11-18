use async_imap::Authenticator;

/// SASL XOAUTH2. A challenge after the first response carries Google's
/// error; the empty answer lets the server finish with `NO`.
pub struct XOAuth2<'a> {
    pub email: &'a str,
    pub token: &'a str,
    pub sent: bool,
}

impl Authenticator for XOAuth2<'_> {
    type Response = String;

    fn process(&mut self, _challenge: &[u8]) -> String {
        if std::mem::replace(&mut self.sent, true) {
            return String::new();
        }

        format!("user={}\x01auth=Bearer {}\x01\x01", self.email, self.token)
    }
}
