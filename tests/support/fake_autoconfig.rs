//! The domain's own autoconfig and Thunderbird's ISPDB inside the test
//! process, over plain HTTP: a document per path, 404 for the rest.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use super::{
    fake_dns::FakeDns,
    fake_http::{FakeHttp, Reply},
};

pub struct FakeAutoconfig {
    http: FakeHttp,
    documents: Arc<Mutex<HashMap<String, String>>>,
}

/// One autoconfig and one DNS that publish nothing, for a test that only
/// needs the guess from the domain.
pub fn nothing_published() -> String {
    static NOTHING: OnceLock<(FakeAutoconfig, FakeDns)> = OnceLock::new();
    let (http, dns) = NOTHING.get_or_init(|| (FakeAutoconfig::start(), FakeDns::start()));
    http.config(dns)
}

impl FakeAutoconfig {
    pub fn start() -> Self {
        let documents = Arc::new(Mutex::new(HashMap::<String, String>::new()));
        let serving = Arc::clone(&documents);
        let http = FakeHttp::start(move |path| {
            serving
                .lock()
                .unwrap()
                .get(path)
                .map_or(Reply::NotFound, |xml| Reply::Body(xml.clone().into_bytes()))
        });

        Self { http, documents }
    }

    /// The `[autoconfig]` table that points Postal at this and `dns`.
    pub fn config(&self, dns: &FakeDns) -> String {
        format!(
            "\n[autoconfig]\ndomain-url = \"http://127.0.0.1:{0}/own/{{domain}}\"\n\
             ispdb-url = \"http://127.0.0.1:{0}/ispdb/{{domain}}\"\ndns-server = \"127.0.0.1:{1}\"\n",
            self.http.port, dns.port
        )
    }

    /// Serves `xml` as the domain's own document for `domain`.
    pub fn publish_own(&self, domain: &str, xml: &str) {
        self.publish(&format!("/own/{domain}"), xml);
    }

    pub fn publish_ispdb(&self, domain: &str, xml: &str) {
        self.publish(&format!("/ispdb/{domain}"), xml);
    }

    fn publish(&self, path: &str, xml: &str) {
        let mut documents = self.documents.lock().unwrap();
        documents.insert(path.to_owned(), xml.to_owned());
    }

    /// The path of each request, in order.
    pub fn paths(&self) -> Vec<String> {
        self.http.paths()
    }
}
