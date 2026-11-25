//! `postal setup-mailto` makes Postal the system's mailto: handler, and a
//! link reaches `postal` in a terminal as one unchanged argument.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    ffi::OsStr,
    fs::{self, File},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use support::home::Home;

const POSTAL: &str = env!("CARGO_BIN_EXE_postal");

/// Links a web page could hold that a shell would split or run.
const HOSTILE: [&str; 5] = [
    "mailto:o'brien@example.com?subject=it's",
    "mailto:bob@example.com?subject=\"quoted\"",
    "mailto:bob@example.com?subject=$(touch pwned)`id`",
    "mailto:bob@example.com?subject=two words  apart",
    "mailto:bob@example.com?subject=a;rm -rf ~;b",
];

fn home(config: &str) -> Home {
    Home::new(&format!("{config}\n[secrets]\nstore = \"file\"\n"))
}

/// A program at `dir/name` that writes each argument it is given, NUL
/// ended, to `dir/name.args`.
fn recorder(dir: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    fs::write(
        &path,
        "#!/bin/sh\nfor a; do printf '%s\\0' \"$a\"; done > \"$0.args\"\n",
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn recorded(program: &Path) -> Vec<String> {
    let mut args = program.as_os_str().to_owned();
    args.push(".args");
    let text = fs::read_to_string(args).unwrap();
    text.split_terminator('\0').map(str::to_owned).collect()
}

/// `program` on `home`, its `PATH` the stubs in `bin` before the system's.
fn program(home: &Home, bin: &Path, program: &Path) -> Command {
    let mut path = bin.as_os_str().to_owned();
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap());
    let mut command = home.command(program.to_str().unwrap());
    command.env("PATH", path);
    command
}

fn postal(home: &Home, bin: &Path) -> Command {
    program(home, bin, Path::new(POSTAL))
}

fn ran(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    output
}

fn canonical(path: impl AsRef<OsStr>) -> PathBuf {
    Path::new(path.as_ref()).canonicalize().unwrap()
}

#[test]
fn an_own_command_is_given_postal_and_each_link_unchanged() {
    let scratch = tempfile::tempdir().unwrap();
    let own = recorder(scratch.path(), "own");
    let home = home(&format!(
        "terminal = {{ command = \"{}\" }}\n",
        own.display()
    ));
    for link in HOSTILE {
        ran(postal(&home, scratch.path()).args(["open-in-terminal", link]));

        let args = recorded(&own);
        assert_eq!(args.len(), 2, "{link}: {args:?}");
        assert_eq!(canonical(&args[0]), canonical(POSTAL), "{link}");
        assert_eq!(args[1], link);
    }
}

#[test]
fn a_terminal_app_is_opened_with_postal_and_the_link_as_its_arguments() {
    let scratch = tempfile::tempdir().unwrap();
    let open = recorder(scratch.path(), "open");
    for (terminal, before) in [
        ("ghostty", &["-na", "Ghostty", "--args", "-e"][..]),
        ("kitty", &["-na", "kitty", "--args"]),
        ("wezterm", &["-na", "WezTerm", "--args", "start", "--"]),
    ] {
        let home = home(&format!("terminal = \"{terminal}\"\n"));
        for link in HOSTILE {
            ran(postal(&home, scratch.path()).args(["open-in-terminal", link]));

            let args = recorded(&open);
            assert_eq!(args.len(), before.len() + 2, "{terminal}: {args:?}");
            assert_eq!(args[..before.len()], *before, "{terminal}");
            assert_eq!(canonical(&args[before.len()]), canonical(POSTAL));
            assert_eq!(args[before.len() + 1], link, "{terminal}");
        }
    }
}

// Terminal.app and iTerm2 only run a file, so the link waits beside it:
// the script holds no link for a shell to parse, and runs once.
#[test]
fn terminal_app_runs_a_script_that_opens_the_link_in_postal_once() {
    let scratch = tempfile::tempdir().unwrap();
    let open = recorder(scratch.path(), "open");
    for (terminal, app) in [("terminal", "Terminal"), ("iterm2", "iTerm")] {
        let home = home(&format!(
            "email = \"me@example.com\"\nterminal = \"{terminal}\"\n"
        ));
        let data = home.xdg_data_home().join("postal");
        fs::create_dir_all(&data).unwrap();
        // The TUI's lock, so the Draft is composed beside it.
        let lock = File::create(data.join("postal.lock")).unwrap();
        lock.lock().unwrap();
        let seen = home.xdg_config_home().join("seen");

        for link in HOSTILE {
            ran(postal(&home, scratch.path()).args(["open-in-terminal", link]));
            let args = recorded(&open);
            assert_eq!(args[..2], ["-a", app], "{args:?}");
            let script = PathBuf::from(&args[2]);
            assert!(!fs::read_to_string(&script).unwrap().contains("mailto"));

            let editor = format!("sh -c 'cp \"$0\" {}'", seen.display());
            ran(program(&home, scratch.path(), &script).env("EDITOR", editor));
            assert!(!script.parent().unwrap().exists(), "{link}");

            let subject = link.split_once("subject=").unwrap().1;
            let opened = fs::read_to_string(&seen).unwrap();
            assert!(
                opened.contains(&format!("\nSubject: {subject}\n")),
                "{link}: {opened}"
            );
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn a_build_without_the_launcher_says_so_and_installs_nothing() {
    let scratch = tempfile::tempdir().unwrap();
    let home = home("");
    let output = postal(&home, scratch.path())
        .env("HOME", scratch.path())
        .arg("setup-mailto")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("without the mailto: launcher"), "{stderr}");
    assert!(!scratch.path().join("Applications").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn on_linux_a_desktop_entry_runs_postal_on_the_link_in_a_terminal() {
    let scratch = tempfile::tempdir().unwrap();
    let mime = recorder(scratch.path(), "xdg-mime");
    let home = home("");
    let entry = home.xdg_data_home().join("applications/postal.desktop");
    // A path the Exec quoting must escape at both its levels.
    let odd = scratch.path().join("a \"b`$c\\d%e");
    fs::create_dir_all(&odd).unwrap();
    let copy = odd.join("postal");
    fs::copy(POSTAL, &copy).unwrap();

    let output = ran(&mut program(&home, scratch.path(), &copy).arg("setup-mailto"));
    let said = String::from_utf8(output.stdout).unwrap();
    assert!(said.contains(entry.to_str().unwrap()), "{said}");
    assert_eq!(
        fs::read_to_string(&entry).unwrap(),
        format!(
            "[Desktop Entry]\nType=Application\nName=Postal\n\
             Comment=Write the mail a mailto: link holds\nExec=\"{}/a \\\\\"b\\\\`\\\\$c\\\\\\\\d%%e/postal\" %u\n\
             Terminal=true\nNoDisplay=true\nMimeType=x-scheme-handler/mailto;\n",
            scratch.path().display()
        )
    );
    assert_eq!(
        recorded(&mime),
        ["default", "postal.desktop", "x-scheme-handler/mailto"]
    );

    ran(postal(&home, scratch.path()).args(["setup-mailto", "--remove"]));
    assert!(!entry.exists());
}
