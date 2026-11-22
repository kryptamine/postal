use crossterm::event::KeyEvent;

use crate::{
    domain::body_key::BodyKey,
    domain::message::Message,
    domain::message_ref::MessageRef,
    domain::per_account::AccountId,
    ui::outside::wanted_attachments::{Purpose, Wanted},
    ui::preview::hint_keys::{HintKeys, Typed},
    ui::state::toast::Toast,
    ui::state::ui::Ui,
};

/// Hint mode on one Message's attachment line: a label before each
/// attachment, and typing one picks it for `purpose`.
pub struct AttachmentHints {
    account: AccountId,
    body_key: BodyKey,
    message: MessageRef,
    /// Each attachment's name and size, in the line's order.
    attachments: Vec<(String, u64)>,
    purpose: Purpose,
    keys: HintKeys,
}

/// What one key does in hint mode; every key ends it but a label's first
/// letter.
pub enum Picked {
    Partial,
    Wanted(Wanted),
    Cancel,
}

/// Enters hint mode on the attachments of the selected Message, in Reading
/// of the one at the cursor, or says why it cannot.
pub fn label_attachments(ui: &mut Ui, purpose: Purpose) {
    let Ui { view, screen } = ui;
    let message = screen.showing.reading().map_or_else(
        || view.selected_message(),
        |reading| view.stack().find(|m| m.body_key == reading.cursor),
    );

    let Some(message) = message else {
        return;
    };

    if message.attachments.is_empty() {
        screen.toast = Some(Toast::new("no attachments"));
        return;
    }

    // One found only with its Thread has no UID here to fetch by.
    let Some(on_server) = message.on_server() else {
        screen.toast = Some(Toast::new("open the Message from its Folder"));
        return;
    };

    let attachments: Vec<_> = message
        .attachments
        .iter()
        .map(|a| (a.name.to_owned(), a.size))
        .collect();
    screen.over.open(AttachmentHints {
        account: message.account,
        body_key: message.body_key,
        message: on_server,
        keys: HintKeys::new(attachments.len()),
        attachments,
        purpose,
    });
}

impl AttachmentHints {
    /// The labels of `message`'s attachment line, when it is the one hinted.
    pub fn labels(&self, message: &Message) -> Option<&HintKeys> {
        (message.account == self.account && message.body_key == self.body_key).then_some(&self.keys)
    }

    pub fn key(&mut self, key: KeyEvent) -> Picked {
        match self.keys.key(key) {
            Typed::Partial => Picked::Partial,
            Typed::Cancel => Picked::Cancel,
            Typed::Picked(index) => {
                let (name, size) = &self.attachments[index];
                Picked::Wanted(Wanted {
                    account: self.account,
                    message: self.message.clone(),
                    index,
                    name: name.clone(),
                    size: *size,
                    purpose: self.purpose,
                })
            }
        }
    }
}
