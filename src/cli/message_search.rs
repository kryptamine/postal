use std::error::Error;

use chrono::Local;

use crate::{
    account::xdg,
    cache::Reader,
    cli::answer::{Answer, Answers},
    cli::args::{LIMIT, MessageSearch},
    cli::thread_rows::{self, ThreadRows},
    domain::query::Query,
};

/// `postal message search`: the newest Threads with a Message matching the
/// Query, words and operators, as the TUI's Search finds them (ADR 0023,
/// ADR 0031), newest first.
impl Answers for MessageSearch {
    fn answer(&self) -> Result<Answer, Box<dyn Error>> {
        let query = Query::parse(&self.query, Local::now());
        if query.terms.is_empty() {
            return Err("the Query has no word or operator to search by".into());
        }

        let reader = Reader::open(&xdg::cache_dir()?)?;
        let mut threads = ThreadRows::new(self.limit.unwrap_or(LIMIT));
        reader.matching(&query, |listed| threads.visit(listed))?;
        let rows = threads.into_rows();
        Ok(Answer::new(&rows, thread_rows::table(&rows))?)
    }
}
