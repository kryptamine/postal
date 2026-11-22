//! Thunderbird autoconfig documents, as a fake autoconfig serves them.

/// A `clientConfig` document listing `servers`, each an `incomingServer`
/// or `outgoingServer` element as Thunderbird writes it.
pub fn client_config(servers: &[String]) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<clientConfig version=\"1.1\">\n\
         <emailProvider id=\"example.com\">\n<domain>example.com</domain>\n{}\n\
         </emailProvider>\n</clientConfig>\n",
        servers.concat()
    )
}

/// An `incomingServer type="imap"` element.
pub fn incoming(host: &str, port: u16, socket: &str, username: &str) -> String {
    server(("incomingServer", "imap"), host, port, socket, username)
}

/// An `outgoingServer type="smtp"` element.
pub fn outgoing(host: &str, port: u16, socket: &str, username: &str) -> String {
    server(("outgoingServer", "smtp"), host, port, socket, username)
}

fn server(
    (tag, kind): (&str, &str),
    host: &str,
    port: u16,
    socket: &str,
    username: &str,
) -> String {
    format!(
        "<{tag} type=\"{kind}\">\n<hostname>{host}</hostname>\n<port>{port}</port>\n\
         <socketType>{socket}</socketType>\n<username>{username}</username>\n\
         <authentication>password-cleartext</authentication>\n</{tag}>\n"
    )
}
