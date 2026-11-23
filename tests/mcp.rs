//! `postal mcp` serves the CLI's commands to an agent as MCP tools on its
//! stdio, one to one: the same arguments, the `--json` array under
//! `items`, a failure as the CLI words it. See ADR 0030.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use serde_json::{Value, json};
use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::Key;

/// A client speaking JSON-RPC to `postal mcp`, one message per line.
struct Client {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
}

impl Client {
    fn start(home: &Home) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_postal"))
            .arg("mcp")
            .env_clear()
            .env("XDG_CONFIG_HOME", home.xdg_config_home())
            .env("XDG_STATE_HOME", home.xdg_state_home())
            .env("XDG_CACHE_HOME", home.xdg_cache_home())
            .env("TZ", "UTC")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut client = Self {
            child,
            input,
            output,
            next: 1,
        };

        let init = client.request(
            "initialize",
            &json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" }
            }),
        );
        assert_eq!(init["serverInfo"]["name"], "postal");
        client.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        client
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.input, "{message}").unwrap();
        self.input.flush().unwrap();
    }

    /// The result of `method`, skipping any notification before it.
    fn request(&mut self, method: &str, params: &Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            assert!(
                self.output.read_line(&mut line).unwrap() > 0,
                "the server hung up"
            );
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                assert!(message["error"].is_null(), "{message}");
                return message["result"].clone();
            }
        }
    }

    fn call(&mut self, tool: &str, arguments: &Value) -> Value {
        self.request(
            "tools/call",
            &json!({ "name": tool, "arguments": arguments }),
        )
    }

    /// Closing stdin ends the server.
    fn close(self) {
        let Self {
            mut child, input, ..
        } = self;
        drop(input);
        assert!(child.wait().unwrap().success());
    }
}

fn inbox() -> Vec<FakeMessage> {
    vec![
        FakeMessage {
            from_address: "ada@example.com",
            ..FakeMessage::new("Ada Lovelace", "Weekly notes", "2024-11-18T09:00:00Z")
        }
        .body(FakeBody::plain("The notes")),
        FakeMessage::new("Bob", "Invoice 1042", "2024-11-16T09:00:00Z")
            .body(FakeBody::plain("Please pay")),
    ]
}

#[test]
fn each_command_is_a_tool_with_its_arguments() {
    let server = FakeImap::start(inbox());
    let home = Home::for_server(&server);
    let mut client = Client::start(&home);
    let tools = client.request("tools/list", &json!({}));
    let tools = tools["tools"].as_array().unwrap();
    let mut names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "correspondent_list",
            "folder_list",
            "message_list",
            "message_search",
            "message_show",
            "thread_show"
        ]
    );

    // A tool's parameters are the command's arguments, `--json` aside.
    let list = tools.iter().find(|tool| tool["name"] == "message_list");
    let mut parameters: Vec<&String> = list.unwrap()["inputSchema"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    parameters.sort_unstable();
    assert_eq!(parameters, ["account", "folder", "limit"]);
    client.close();
}

#[test]
fn a_tool_answers_as_its_command_beside_an_open_tui() {
    let server = FakeImap::start(inbox());
    let home = Home::for_server(&server);
    let mut t = home.terminal().spawn(env!("CARGO_BIN_EXE_postal")).unwrap();
    t.wait_until(|s| s.contains("The notes")).unwrap();

    let mut client = Client::start(&home);
    let listed = client.call("message_list", &json!({}));
    assert_eq!(listed["isError"], false);
    let rows = listed["structuredContent"]["items"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let text = listed["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Invoice 1042"), "{text}");

    let shown = client.call("message_show", &json!({ "id": rows[0]["message"] }));
    assert_eq!(shown["structuredContent"]["items"][0]["body"], "The notes");
    let thread = client.call("thread_show", &json!({ "id": rows[0]["thread"] }));
    let subject = &thread["structuredContent"]["items"][0]["subject"];
    assert_eq!(subject, "Weekly notes");
    let found = client.call("message_search", &json!({ "query": "invoice" }));
    let subject = &found["structuredContent"]["items"][0]["subject"];
    assert_eq!(subject, "Invoice 1042");

    // A failure is the tool's, and the server keeps serving.
    let refused = client.call("message_show", &json!({ "id": "nonsense" }));
    assert_eq!(refused["isError"], true);
    assert_eq!(refused["content"][0]["text"], "nonsense is no Message ID");
    let folders = client.call("folder_list", &json!({}));
    assert_eq!(folders["isError"], false);
    let known = client.call("correspondent_list", &json!({}));
    let addresses = text_of(&known, "address");
    assert!(addresses.contains(&"ada@example.com".to_owned()), "{known}");
    // `id` and `draft` exclude each other, as the flags do.
    let both = client.call(
        "thread_show",
        &json!({ "id": rows[0]["thread"], "draft": "x" }),
    );
    assert_eq!(both["isError"], true);
    client.close();

    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn text_of(result: &Value, field: &str) -> Vec<String> {
    let items = result["structuredContent"]["items"].as_array().unwrap();
    items
        .iter()
        .map(|item| item[field].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn without_a_cache_a_tool_says_so() {
    let server = FakeImap::start(inbox());
    let home = Home::for_server(&server);
    let mut client = Client::start(&home);
    let listed = client.call("message_list", &json!({}));
    assert_eq!(listed["isError"], true);
    assert_eq!(
        listed["content"][0]["text"],
        "Cache not ready: run `postal` once"
    );
    client.close();
}
