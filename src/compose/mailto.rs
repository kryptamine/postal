/// A `mailto:` link (RFC 6068) as a new Draft takes it. Every header but
/// `to`, `cc`, `subject` and `body` is dropped: a link from a web page
/// must not choose the sender or attach a file.
pub struct Mailto {
    to: Vec<String>,
    cc: Vec<String>,
    subject: String,
    body: String,
    /// A Draft has no Bcc, so one in the link is dropped, and said so.
    pub dropped_bcc: bool,
}

impl Mailto {
    /// The error says what in the link does not parse.
    pub fn parse(link: &str) -> Result<Self, String> {
        let rest = link
            .get(..7)
            .filter(|scheme| scheme.eq_ignore_ascii_case("mailto:"))
            .and_then(|_| link.get(7..))
            .ok_or_else(|| format!("neither a command nor a mailto: link: {link}"))?;
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let mut mailto = Self {
            to: recipients(&decoded(path)?).into_iter().collect(),
            cc: Vec::new(),
            subject: String::new(),
            body: String::new(),
            dropped_bcc: false,
        };

        for field in query.split('&').filter(|field| !field.is_empty()) {
            let (name, value) = field
                .split_once('=')
                .ok_or_else(|| format!("`{field}` in the link has no `=`"))?;
            let value = decoded(value)?;
            match decoded(name)?.to_ascii_lowercase().as_str() {
                "to" => mailto.to.extend(recipients(&value)),
                "cc" => mailto.cc.extend(recipients(&value)),
                "subject" if mailto.subject.is_empty() => mailto.subject = one_line(&value),
                "body" if mailto.body.is_empty() => {
                    mailto.body = value.replace("\r\n", "\n").replace('\r', "\n");
                }
                "bcc" => mailto.dropped_bcc |= !value.trim().is_empty(),
                _ => {}
            }
        }

        Ok(mailto)
    }

    /// The Draft's file as the editor opens it, `signature` below the body
    /// as in the template.
    pub fn draft(&self, signature: Option<&str>) -> String {
        let (to, cc) = (self.to.join(", "), self.cc.join(", "));
        let mut text = format!("To: {to}\nCc: {cc}\nSubject: {}\n\n", self.subject);
        if !self.body.is_empty() {
            text.push_str(self.body.trim_end_matches('\n'));
            text.push('\n');
        }

        if let Some(signature) = signature {
            text.push('\n');
            text.push_str(signature);
        }

        text
    }
}

/// A comma-separated list of addresses, kept whole for a quoted name's
/// comma, on one line.
fn recipients(list: &str) -> Option<String> {
    Some(one_line(list).trim().to_owned()).filter(|list| !list.is_empty())
}

// A Draft's head is header lines: a decoded line break would add a header,
// such as an `Attach:` naming the user's key. Other controls go with it.
fn one_line(value: &str) -> String {
    value.replace(char::is_control, " ")
}

/// `text` percent-decoded, as UTF-8; `+` stays a plus.
fn decoded(text: &str) -> Result<String, String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let Some((&byte, after)) = rest.split_first() {
        rest = after;
        if byte != b'%' {
            bytes.push(byte);
            continue;
        }

        let (hex, after) = rest
            .split_first_chunk::<2>()
            .filter(|(hex, _)| hex.iter().all(u8::is_ascii_hexdigit))
            .ok_or_else(|| format!("a bad percent-escape in the link: {text}"))?;
        let hex = std::str::from_utf8(hex).map_err(|error| error.to_string())?;
        bytes.push(u8::from_str_radix(hex, 16).map_err(|error| error.to_string())?);
        rest = after;
    }

    String::from_utf8(bytes).map_err(|_bad| format!("the link is not UTF-8: {text}"))
}
