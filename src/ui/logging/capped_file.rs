use std::{
    fs::File,
    io::{self, Write},
};

/// A log file that stops growing at `cap` bytes, ending with one `log truncated` line.
pub struct CappedFile {
    file: File,
    written: u64,
    cap: u64,
}

impl CappedFile {
    pub const fn new(file: File, cap: u64) -> Self {
        Self {
            file,
            written: 0,
            cap,
        }
    }
}

impl Write for CappedFile {
    // Every event arrives as one `write`, so an event is kept or dropped whole.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written >= self.cap {
            return Ok(buf.len());
        }

        let len = u64::try_from(buf.len()).unwrap_or(u64::MAX);
        if self.written + len > self.cap {
            self.written = self.cap;
            self.file.write_all(b"log truncated\n")?;
        } else {
            self.written += len;
            self.file.write_all(buf)?;
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
