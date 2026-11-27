use termlens::{Key, Terminal};

/// Quits with `Ctrl-c`, which every view takes, and checks the exit.
pub fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}
