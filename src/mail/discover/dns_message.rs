//! A DNS query for SRV records and its answer, as RFC 1035 lays them out.

const SRV: u16 = 33;
const INTERNET: u16 = 1;
/// Labels and pointers in one name; past it, a pointer loop is taken.
const MAX_STEPS: usize = 128;

/// One SRV record (RFC 2782); an empty `target` says there is no such service.
pub struct Srv {
    pub priority: u16,
    pub weight: u16,
    pub port: u16,
    pub target: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the name {0:?} has a label DNS cannot carry")]
    Name(String),
    #[error("the answer is malformed")]
    Malformed,
    #[error("the answer was truncated")]
    Truncated,
    #[error("the server answered with code {0}")]
    Refused(u8),
}

/// A query numbered `id` for the SRV records of `name`, recursion desired.
pub fn query(id: u16, name: &str) -> Result<Vec<u8>, Error> {
    let mut message = Vec::with_capacity(18 + name.len());
    message.extend(id.to_be_bytes());
    message.extend([0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
    for label in name.trim_end_matches('.').split('.') {
        let length = u8::try_from(label.len())
            .ok()
            .filter(|length| (1..=63).contains(length))
            .ok_or_else(|| Error::Name(name.to_owned()))?;
        message.push(length);
        message.extend(label.as_bytes());
    }

    message.push(0);
    message.extend(SRV.to_be_bytes());
    message.extend(INTERNET.to_be_bytes());
    Ok(message)
}

/// The SRV records of an answer to query `id`; `None` when it answers
/// another. A name that does not exist has none.
pub fn answer(id: u16, message: &[u8]) -> Result<Option<Vec<Srv>>, Error> {
    let mut reader = Reader { message, at: 0 };
    if reader.u16()? != id {
        return Ok(None);
    }

    let [flags, code] = [reader.u8()?, reader.u8()?];
    if flags & 0x80 == 0 {
        return Ok(None);
    }

    if flags & 0x02 != 0 {
        return Err(Error::Truncated);
    }

    match code & 0x0f {
        0 => {}
        3 => return Ok(Some(Vec::new())),
        code => return Err(Error::Refused(code)),
    }

    let questions = reader.u16()?;
    let answers = reader.u16()?;
    reader.skip(4)?;
    for _ in 0..questions {
        reader.name()?;
        reader.skip(4)?;
    }

    let mut records = Vec::new();
    for _ in 0..answers {
        reader.name()?;
        let (kind, class) = (reader.u16()?, reader.u16()?);
        reader.skip(4)?;
        let length = usize::from(reader.u16()?);
        let end = reader.at + length;
        if kind == SRV && class == INTERNET {
            records.push(Srv {
                priority: reader.u16()?,
                weight: reader.u16()?,
                port: reader.u16()?,
                target: reader.name()?,
            });
        }

        reader.at = end;
    }

    Ok(Some(records))
}

struct Reader<'m> {
    message: &'m [u8],
    at: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Result<u8, Error> {
        let byte = *self.message.get(self.at).ok_or(Error::Malformed)?;
        self.at += 1;
        Ok(byte)
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_be_bytes([self.u8()?, self.u8()?]))
    }

    const fn skip(&mut self, bytes: usize) -> Result<(), Error> {
        self.at += bytes;
        if self.at > self.message.len() {
            return Err(Error::Malformed);
        }

        Ok(())
    }

    /// A name, its labels joined by dots, following compression pointers;
    /// the root is empty.
    fn name(&mut self) -> Result<String, Error> {
        let mut labels: Vec<String> = Vec::new();
        let mut at = self.at;
        let mut resume = None;
        for _ in 0..MAX_STEPS {
            let length = *self.message.get(at).ok_or(Error::Malformed)?;
            match length {
                0 => {
                    self.at = resume.unwrap_or(at + 1);
                    return Ok(labels.join("."));
                }
                0xc0.. => {
                    let low = *self.message.get(at + 1).ok_or(Error::Malformed)?;
                    resume.get_or_insert(at + 2);
                    at = usize::from(u16::from_be_bytes([length & 0x3f, low]));
                }
                1..=63 => {
                    let start = at + 1;
                    let end = start + usize::from(length);
                    let label = self.message.get(start..end).ok_or(Error::Malformed)?;
                    labels.push(String::from_utf8_lossy(label).into_owned());
                    at = end;
                }
                _ => return Err(Error::Malformed),
            }
        }

        Err(Error::Malformed)
    }
}
