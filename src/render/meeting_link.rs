/// Hosts of video calls, whose link in an event's description is the one to
/// join by; a description holds other links too (Google's own, for one).
const MEETING_HOSTS: &[&str] = &[
    "meet.google.com",
    "zoom.us",
    "teams.microsoft.com",
    "teams.live.com",
    "webex.com",
    "whereby.com",
    "meet.jit.si",
];

/// Each `http` or `https` URL in `text`, in order.
pub fn urls(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"'))
        .filter_map(|word| {
            let at = word.find("https://").or_else(|| word.find("http://"))?;
            let url = word
                .get(at..)?
                .trim_end_matches(['.', ',', ';', ')', ']', '\'']);
            Some(url)
        })
}

/// Whether `url` joins a video call.
pub fn meeting(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#', ':']).next().unwrap_or_default();
    MEETING_HOSTS.iter().any(|known| {
        host.eq_ignore_ascii_case(known)
            || host
                .len()
                .checked_sub(known.len() + 1)
                .and_then(|dot| host.get(dot..))
                .is_some_and(|tail| tail.eq_ignore_ascii_case(&format!(".{known}")))
    })
}

/// `text` without its URLs and the blanks they leave.
pub fn without_urls(text: &str) -> String {
    let mut kept = text.to_owned();
    for url in urls(text) {
        kept = kept.replacen(url, "", 1);
    }

    let words: Vec<&str> = kept.split_whitespace().collect();
    words
        .join(" ")
        .trim_matches([',', ';', '-', ' '])
        .to_owned()
}
