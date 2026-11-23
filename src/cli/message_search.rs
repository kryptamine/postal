use std::error::Error;

use crate::{
    account::xdg,
    cache::Reader,
    cli::answer::{Answer, Answers},
    cli::args::{LIMIT, MessageSearch},
    cli::thread_rows::{self, ThreadRows},
};

/// `postal message search`: the newest Threads with a Message, in any
/// Folder but Trash and Spam, whose words start with each word of the
/// Query, as the TUI's Search finds them (ADR 0023), newest first.
impl Answers for MessageSearch {
    fn answer(&self) -> Result<Answer, Box<dyn Error>> {
        if !self.query.chars().any(char::is_alphanumeric) {
            return Err("the Query has no word to search for".into());
        }

        let reader = Reader::open(&xdg::cache_dir()?)?;
        let mut threads = ThreadRows::new(self.limit.unwrap_or(LIMIT));
        reader.matching(&self.query, |listed| threads.visit(listed))?;
        let rows = threads.into_rows();
        Ok(Answer::new(&rows, thread_rows::table(&rows))?)
    }
}
