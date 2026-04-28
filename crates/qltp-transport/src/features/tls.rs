//! TLS/SSL encryption support for QLTP
//!
//! This module provides TLS encryption for secure file transfers using rustls.

use crate::error::{Error, Result};
use rustls::{ClientConfig, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// TLS configuration for client connections
#[derive(Clone)]
pub struct TlsClientConfig {
    config: Arc<ClientConfig>,
}

impl TlsClientConfig {
    /// Create a new TLS client configuration with full peer verification.
    ///
    /// SECURITY: This is the ONLY public constructor. Peer-cert and hostname
    /// verification are always enabled. The previous "verify_hostname=false"
    /// path installed a `NoVerifier` that accepted every certificate and
    /// every signature, which trivially defeats TLS (CWE-295). It has been
    /// removed entirely. Test code that needs a no-verify client must use
    /// the `#[cfg(test)] new_dangerous_accept_any` constructor below.
    ///
    /// # Arguments
    /// * `ca_cert_path` - Path to CA certificate file (optional, uses webpki
    ///   system roots if `None`).
    pub fn new(ca_cert_path: Option<&Path>) -> Result<Self> {
        let root_store = Self::build_root_store(ca_cert_path)?;
        let config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        Ok(Self {
            config: Arc::new(config),
        })
    }

    /// Build a root certificate store from an optional CA file (or webpki
    /// roots if `None`). Shared between the secure and the test-only
    /// constructors so loading logic is identical.
    fn build_root_store(ca_cert_path: Option<&Path>) -> Result<rustls::RootCertStore> {
        let mut root_store = rustls::RootCertStore::empty();
        if let Some(ca_path) = ca_cert_path {
            let ca_file = File::open(ca_path)
                .map_err(|e| Error::Tls(format!("Failed to open CA cert: {}", e)))?;
            let mut ca_reader = BufReader::new(ca_file);

            let ca_certs = certs(&mut ca_reader)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| Error::Tls(format!("Failed to parse CA cert: {}", e)))?;

            for cert in ca_certs {
                root_store
                    .add(cert)
                    .map_err(|e| Error::Tls(format!("Failed to add CA cert: {}", e)))?;
            }
        } else {
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        }
        Ok(root_store)
    }

    /// Test-only constructor that disables certificate verification by
    /// installing `NoVerifier`. Compiled out of release builds and not
    /// callable from non-test code in any other crate.
    ///
    /// SECURITY: Never expose this through any other API.
    #[cfg(test)]
    pub fn new_dangerous_accept_any() -> Result<Self> {
        let root_store = Self::build_root_store(None)?;
        let mut config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        config
            .dangerous()
            .set_certificate_verifier(Arc::new(NoVerifier));
        Ok(Self {
            config: Arc::new(config),
        })
    }

    /// Create a TLS connector
    pub fn connector(&self) -> TlsConnector {
        TlsConnector::from(self.config.clone())
    }

    /// Connect to a server with TLS
    pub async fn connect(
        &self,
        stream: TcpStream,
        server_name: &str,
    ) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
        let connector = self.connector();
        let domain = rustls::pki_types::ServerName::try_from(server_name.to_string())
            .map_err(|e| Error::Tls(format!("Invalid server name: {}", e)))?;

        connector
            .connect(domain, stream)
            .await
            .map_err(|e| Error::Tls(format!("TLS connection failed: {}", e)))
    }
}

/// TLS configuration for server connections
#[derive(Clone)]
pub struct TlsServerConfig {
    config: Arc<ServerConfig>,
}

impl TlsServerConfig {
    /// Create a new TLS server configuration
    ///
    /// # Arguments
    /// * `cert_path` - Path to server certificate file
    /// * `key_path` - Path to server private key file
    pub fn new(cert_path: &Path, key_path: &Path) -> Result<Self> {
        // Load certificate chain
        let cert_file = File::open(cert_path)
            .map_err(|e| Error::Tls(format!("Failed to open cert file: {}", e)))?;
        let mut cert_reader = BufReader::new(cert_file);
        
        let certs = certs(&mut cert_reader)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Tls(format!("Failed to parse cert: {}", e)))?;

        // Load private key
        let key_file = File::open(key_path)
            .map_err(|e| Error::Tls(format!("Failed to open key file: {}", e)))?;
        let mut key_reader = BufReader::new(key_file);
        
        let mut keys = pkcs8_private_keys(&mut key_reader)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Tls(format!("Failed to parse key: {}", e)))?;

        if keys.is_empty() {
            return Err(Error::Tls("No private keys found".to_string()));
        }

        let key = keys.remove(0);

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key.into())
            .map_err(|e| Error::Tls(format!("Failed to create server config: {}", e)))?;

        Ok(Self {
            config: Arc::new(config),
        })
    }

    /// Create a TLS acceptor
    pub fn acceptor(&self) -> TlsAcceptor {
        TlsAcceptor::from(self.config.clone())
    }

    /// Accept a TLS connection
    pub async fn accept(&self, stream: TcpStream) -> Result<tokio_rustls::server::TlsStream<TcpStream>> {
        let acceptor = self.acceptor();
        acceptor
            .accept(stream)
            .await
            .map_err(|e| Error::Tls(format!("TLS accept failed: {}", e)))
    }
}

/// Certificate verifier that accepts ANY certificate.
///
/// SECURITY: This struct is `#[cfg(test)]`-only and cannot be reached from
/// any release build. It exists solely to support the
/// `TlsClientConfig::new_dangerous_accept_any` constructor used by
/// integration tests against self-signed peers.
#[cfg(test)]
#[derive(Debug)]
struct NoVerifier;

#[cfg(test)]
impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

/// Generate self-signed certificate for testing
#[cfg(test)]
pub fn generate_self_signed_cert() -> Result<(Vec<u8>, Vec<u8>)> {
    use rcgen::{Certificate, CertificateParams, DistinguishedName};

    let mut params = CertificateParams::new(vec!["localhost".to_string()]);
    
    let mut dn = DistinguishedName::new();
    dn.push(rcgen::DnType::CommonName, "QLTP Test Server");
    params.distinguished_name = dn;

    let cert = Certificate::from_params(params)
        .map_err(|e| Error::Tls(format!("Failed to generate cert: {}", e)))?;

    let cert_pem = cert.serialize_pem()
        .map_err(|e| Error::Tls(format!("Failed to serialize cert: {}", e)))?;
    
    let key_pem = cert.serialize_private_key_pem();

    Ok((cert_pem.into_bytes(), key_pem.into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_generate_self_signed_cert() {
        let result = generate_self_signed_cert();
        assert!(result.is_ok());
        
        let (cert, key) = result.unwrap();
        assert!(!cert.is_empty());
        assert!(!key.is_empty());
    }

    #[test]
    fn test_tls_server_config() {
        let (cert_pem, key_pem) = generate_self_signed_cert().unwrap();
        
        let mut cert_file = NamedTempFile::new().unwrap();
        cert_file.write_all(&cert_pem).unwrap();
        cert_file.flush().unwrap();
        
        let mut key_file = NamedTempFile::new().unwrap();
        key_file.write_all(&key_pem).unwrap();
        key_file.flush().unwrap();
        
        let config = TlsServerConfig::new(cert_file.path(), key_file.path());
        assert!(config.is_ok());
    }

    #[test]
    fn test_tls_client_config() {
        // Test with system roots
        let config = TlsClientConfig::new(None);
        assert!(config.is_ok());

        // Test the cfg(test)-only insecure constructor.
        let config = TlsClientConfig::new_dangerous_accept_any();
        assert!(config.is_ok());
    }
}

// Made with Bob
