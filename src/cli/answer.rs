use std::{error::Error, io};

use serde::Serialize;
use serde_json::Value;

use crate::cli::command_output;

/// A command on mail, as the CLI and its MCP tool run it alike
/// (ADR 0030); its arguments are the tool's parameters.
pub trait Answers {
    fn answer(&self) -> Result<Answer, Box<dyn Error>>;
}

/// What a command answers, the same on both surfaces (ADR 0030): the
/// `--json` array, whose fields only grow, and the text a person reads.
pub struct Answer {
    pub json: Value,
    pub text: String,
    /// Part of the work failed, which the rows say: the command exits with 1.
    pub failed: bool,
}

impl Answer {
    pub fn new(rows: &impl Serialize, text: String) -> serde_json::Result<Self> {
        Ok(Self {
            json: serde_json::to_value(rows)?,
            text,
            failed: false,
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
