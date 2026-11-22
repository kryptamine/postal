use crate::{
    account::config::NotifyChoice, mail::arrival::Arrival, ui::outside::delivery::Delivery,
};

/// Raises a desktop notification for new mail, as `notify` asks.
pub struct Notifier {
    delivery: Delivery,
    pub notify: NotifyChoice,
    /// `None` until the terminal reports focus: one that never does, as
    /// tmux by default, would otherwise never notify.
    focused: Option<bool>,
}

impl Notifier {
    pub fn new(notify: NotifyChoice) -> Self {
        Self {
            delivery: Delivery::from_env(),
            notify,
            focused: None,
        }
    }

    pub const fn focus(&mut self, focused: bool) {
        self.focused = Some(focused);
    }

    /// One notification for the whole batch: its sender and subject when
    /// alone, else how many came and from whom.
    pub fn arrived(&self, arrivals: &[Arrival]) {
        let wanted = match self.notify {
            NotifyChoice::Unfocused => self.focused != Some(true),
            NotifyChoice::Always => true,
            NotifyChoice::Never => false,
        };

        match arrivals {
            _ if !wanted => {}
            [] => {}
            [one] => self.delivery.send(&one.sender, &one.subject),
            several => {
                let mut senders: Vec<&str> = Vec::new();
                for arrival in several {
                    if !senders.contains(&arrival.sender.as_str()) {
                        senders.push(&arrival.sender);
                    }
                }

                let title = format!("{} new messages", several.len());
                self.delivery.send(&title, &senders.join(", "));
            }
        }
    }
}
