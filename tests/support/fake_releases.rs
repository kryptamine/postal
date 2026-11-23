//! GitHub's releases page inside the test process, over plain HTTP:
//! `latest` redirects to the tag published, and each release file is
//! served at its download path.

use std::{
    collections::HashMap,
    fmt::Write as _,
    fs,
    process::Command,
    sync::{Arc, Mutex},
};

use ring::digest::{SHA256, digest};

use super::fake_http::{FakeHttp, Reply};

/// The archive `make dist` names for the target the tests are built for.
pub const ARCHIVE: &str = concat!("postal-", env!("POSTAL_TARGET"));

#[derive(Default)]
struct Published {
    latest: Option<String>,
    files: HashMap<String, Vec<u8>>,
}

pub struct FakeReleases {
    http: FakeHttp,
    published: Arc<Mutex<Published>>,
}

impl FakeReleases {
    pub fn start() -> Self {
        let published = Arc::new(Mutex::new(Published::default()));
        let serving = Arc::clone(&published);
        let http = FakeHttp::start(move |path| {
            let published = serving.lock().unwrap();
            match (path, &published.latest) {
                ("/releases/latest", Some(tag)) => Reply::Redirect(format!("/releases/tag/{tag}")),
                _ => published
                    .files
                    .get(path)
                    .map_or(Reply::NotFound, |file| Reply::Body(file.clone())),
            }
        });

        Self { http, published }
    }

    /// The value of `POSTAL_RELEASES_URL` that points Postal here.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/releases", self.http.port)
    }

    /// Publishes `tag` as the latest release, with no files.
    pub fn publish_tag(&self, tag: &str) {
        self.published.lock().unwrap().latest = Some(tag.to_owned());
    }

    /// Publishes `tag` as the latest release, its archive holding `script`
    /// as the binary, beside the archive's sha256.
    pub fn publish(&self, tag: &str, script: &str) {
        let archive = pack(script);
        let sha = hex(digest(&SHA256, &archive).as_ref());
        self.publish_tag(tag);
        let mut published = self.published.lock().unwrap();
        published.files.insert(
            checksum_path(tag),
            format!("{sha}  {ARCHIVE}.tar.gz\n").into_bytes(),
        );
        published.files.insert(archive_path(tag), archive);
    }

    /// Makes the sha256 of release `tag` name some other archive.
    pub fn break_checksum(&self, tag: &str) {
        let wrong = format!("{}  {ARCHIVE}.tar.gz\n", "0".repeat(64));
        let mut published = self.published.lock().unwrap();
        published
            .files
            .insert(checksum_path(tag), wrong.into_bytes());
    }

    /// The path of each request, in order.
    pub fn paths(&self) -> Vec<String> {
        self.http.paths()
    }
}

pub fn archive_path(tag: &str) -> String {
    format!("/releases/download/{tag}/{ARCHIVE}.tar.gz")
}

pub fn checksum_path(tag: &str) -> String {
    format!("{}.sha256", archive_path(tag))
}

fn pack(script: &str) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join(ARCHIVE).join("postal");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::write(&binary, script).unwrap();
    let archive = dir.path().join("archive.tar.gz");
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(dir.path())
        .arg(ARCHIVE)
        .status()
        .unwrap();
    assert!(status.success());
    fs::read(archive).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        write!(text, "{byte:02x}").unwrap();
        text
    })
}
