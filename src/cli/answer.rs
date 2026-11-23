use std::io;

use serde::Serialize;
use serde_json::Value;

use crate::cli::command_output;

/// What a command answers, the same on both surfaces (ADR 0030): the
/// `--json` array, whose fields only grow, and the text a person reads.
pub struct Answer {
    pub json: Value,
    pub text: String,
}

impl Answer {
    pub fn new(rows: &impl Serialize, text: String) -> serde_json::Result<Self> {
        Ok(Self {
            json: serde_json::to_value(rows)?,
            text,
        })
    }

    pub fn print(&self, json: bool) -> io::Result<()> {
        if json {
            command_output::print(&format!("{}\n", self.json))
        } else {
            command_output::print(&self.text)
        }
    }
}
