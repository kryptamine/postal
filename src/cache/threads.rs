use rusqlite::params;

use super::{Cache, Error, bodies, correspondence};
use crate::{
    domain::body_key::BodyKey, domain::correspondence::Correspondence, domain::message::Message,
    domain::thread_id::ThreadId,
};

impl Cache {
    /// Replaces what is cached of Gmail's Thread `id` from other Folders
    /// with `messages`, storing `bodies` by key first and learning from
    /// what `learnt` says of them, in one transaction.
    pub fn write_thread(
        &self,
        id: ThreadId,
        messages: &[Message<()>],
        learnt: &[Correspondence],
        bodies: &[(BodyKey, Vec<u8>)],
    ) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            bodies::store(&transaction, self.email(), bodies)?;
            for seen in learnt {
                correspondence::record(&transaction, self.email(), seen)?;
            }

            transaction.execute(
                "DELETE FROM thread_message WHERE account = ?2 AND thread = ?1",
                params![id, self.email()],
            )?;
            let mut insert = transaction.prepare_cached(
                "INSERT OR REPLACE INTO thread_message
                 (key, thread, received, received_offset, unread, sender, subject, from_me,
                  snippet, answered, account, from_line, to_line, attachments)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?;
            for m in messages {
                insert.execute(params![
                    m.body_key,
                    id,
                    m.received.timestamp(),
                    m.received.offset().local_minus_utc(),
                    m.unread,
                    m.sender,
                    m.subject,
                    m.from_me,
                    m.snippet,
                    m.answered,
                    self.email(),
                    m.from,
                    m.to,
                    m.attachments.stored(),
                ])?;
            }

            drop(insert);
            bodies::evict(&transaction)?;
            transaction.commit()
        })
    }
}
