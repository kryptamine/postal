use std::{fmt::Debug, io, sync::Arc};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::{
    TlsConnector,
    rustls::{ClientConfig, RootCertStore, crypto::ring, pki_types::ServerName},
};

use super::Error;
use crate::account::config::Imap;

pub trait Transport: AsyncRead + AsyncWrite + Unpin + Send + Debug {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + Debug> Transport for T {}

/// Opens TCP, wrapped in TLS unless the config allowed plaintext.
pub async fn connect(imap: &Imap) -> Result<Box<dyn Transport>, Error> {
    let address = format!("{}:{}", imap.host, imap.port);
    let connect_error = |source| Error::Connect {
        address: address.clone(),
        source,
    };

    let tcp = TcpStream::connect((imap.host.as_str(), imap.port))
        .await
        .map_err(connect_error)?;
    if !imap.tls {
        return Ok(Box::new(tcp));
    }

    let config = ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| connect_error(io::Error::other(e)))?
        .with_root_certificates(RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        })
        .with_no_client_auth();
    let name = ServerName::try_from(imap.host.clone())
        .map_err(|e| connect_error(io::Error::new(io::ErrorKind::InvalidInput, e)))?;
    let tls = TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await
        .map_err(connect_error)?;
    Ok(Box::new(tls))
}
