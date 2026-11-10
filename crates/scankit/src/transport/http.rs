// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{WebPkiSupportedAlgorithms, verify_tls12_signature, verify_tls13_signature};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme};
use rustls_pki_types::{CertificateDer, ServerName, UnixTime};
use sha2::{Digest, Sha256};
use ureq::http::Uri;
use ureq::tls::{Certificate, RootCerts, TlsConfig};
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, Either, LazyBuffers, NextTimeout, TcpConnector,
    Transport, TransportAdapter,
};

use crate::error::{Error, ErrorKind, Fingerprint, Result};
use crate::wire::{HttpCall, HttpReply, MAX_CONTROL_BODY, Method, StreamReply};

static NATIVE_ROOTS: OnceLock<Vec<Certificate<'static>>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub(crate) struct Options {
    pub(crate) connect_timeout: Duration,
    pub(crate) operation_timeout: Duration,
    pub(crate) pin: Option<Fingerprint>,
}

#[derive(Debug)]
pub(crate) struct HttpTransport {
    agent: ureq::Agent,
    options: Options,
    presented: Arc<Mutex<Option<Fingerprint>>>,
}

impl HttpTransport {
    pub(crate) fn new(options: Options, secure: bool) -> Result<Self> {
        let roots = if options.pin.is_some() || !secure {
            Vec::new()
        } else {
            let roots = NATIVE_ROOTS.get_or_init(|| {
                rustls_native_certs::load_native_certs()
                    .certs
                    .iter()
                    .map(|certificate| Certificate::from_der(certificate.as_ref()).to_owned())
                    .collect()
            });
            if roots.is_empty() {
                return Err(Error::new(
                    ErrorKind::Unavailable,
                    "could not load any certificate from the system trust store",
                ));
            }
            roots.clone()
        };
        let tls = TlsConfig::builder()
            .root_certs(RootCerts::from(roots))
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .build();
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .max_response_header_size(64 * 1024)
            .timeout_connect(Some(options.connect_timeout))
            .timeout_send_request(Some(options.operation_timeout))
            .timeout_send_body(Some(options.operation_timeout))
            .timeout_recv_response(Some(options.operation_timeout))
            .timeout_recv_body(Some(options.operation_timeout))
            .tls_config(tls)
            .user_agent(concat!("scankit/", env!("CARGO_PKG_VERSION")))
            .build();
        let presented = Arc::new(Mutex::new(None));
        let agent = if let Some(expected) = options.pin {
            let connector = ().chain(TcpConnector::default()).chain(PinnedTlsConnector {
                expected,
                presented: Arc::clone(&presented),
            });
            ureq::Agent::with_parts(config, connector, DefaultResolver::default())
        } else {
            config.new_agent()
        };
        Ok(Self {
            agent,
            options,
            presented,
        })
    }

    pub(crate) fn control(&self, call: &HttpCall) -> Result<HttpReply> {
        let response = self.execute(call)?;
        let status = response.status().as_u16();
        let headers = response_headers(&response);
        let (_, body) = response.into_parts();
        let body = body
            .into_with_config()
            .limit(MAX_CONTROL_BODY)
            .read_to_vec()
            .map_err(|error| map_ureq_error("read scanner control response", error))?;
        Ok(HttpReply {
            status,
            headers,
            body,
        })
    }

    pub(crate) fn stream(&self, call: &HttpCall) -> Result<StreamReply> {
        let response = self.execute(call)?;
        let status = response.status().as_u16();
        let headers = response_headers(&response);
        let (_, body) = response.into_parts();
        Ok(StreamReply {
            status,
            headers,
            body: Box::new(body.into_reader()),
        })
    }

    fn execute(&self, call: &HttpCall) -> Result<ureq::http::Response<ureq::Body>> {
        let url = call.target.parse::<Uri>().map_err(|error| {
            Error::with_source(
                ErrorKind::InvalidInput,
                "scanner endpoint is not a valid absolute URL",
                error,
            )
        })?;
        let scheme = url.scheme_str().ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidInput,
                "scanner endpoint has no URL scheme",
            )
        })?;
        if scheme != "http" && scheme != "https" {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "scanner endpoint must use HTTP or HTTPS",
            ));
        }
        if scheme == "http" && self.options.pin.is_some() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "a certificate fingerprint cannot be used with plain HTTP",
            ));
        }
        if self.options.pin.is_some() {
            if let Ok(mut presented) = self.presented.lock() {
                *presented = None;
            }
        }

        let response = match call.method {
            Method::Get => {
                let request = call.headers.iter().fold(
                    self.agent.get(call.target.as_str()),
                    |request, (name, value)| request.header(name, value),
                );
                request.call()
            },
            Method::Delete => {
                let request = call.headers.iter().fold(
                    self.agent.delete(call.target.as_str()),
                    |request, (name, value)| request.header(name, value),
                );
                request.call()
            },
            Method::Post => {
                let request = call.headers.iter().fold(
                    self.agent.post(call.target.as_str()),
                    |request, (name, value)| request.header(name, value),
                );
                request.send(call.body.as_slice())
            },
        };
        response.map_err(|error| {
            if scheme == "https" && is_tls_error(&error) {
                if self.options.pin.is_some() {
                    if let Ok(presented) = self.presented.lock() {
                        if let Some(fingerprint) = *presented {
                            return Error::untrusted(
                                "scanner certificate does not match the pinned fingerprint",
                                fingerprint,
                            );
                        }
                    }
                    return map_ureq_error("establish pinned TLS with scanner", error);
                }
                if let Ok((_, fingerprint)) = peer_certificate(&url, self.options.connect_timeout) {
                    return Error::untrusted(
                        "scanner certificate is not trusted; verify and pin this fingerprint",
                        fingerprint,
                    );
                }
            }
            map_ureq_error("communicate with scanner", error)
        })
    }
}

#[derive(Debug)]
struct PinnedTlsConnector {
    expected: Fingerprint,
    presented: Arc<Mutex<Option<Fingerprint>>>,
}

impl<Incoming: Transport> Connector<Incoming> for PinnedTlsConnector {
    type Out = Either<Incoming, PinnedTlsTransport>;

    fn connect(
        &self,
        details: &ConnectionDetails<'_>,
        chained: Option<Incoming>,
    ) -> std::result::Result<Option<Self::Out>, ureq::Error> {
        let Some(transport) = chained else {
            return Err(ureq::Error::ConnectionFailed);
        };
        if !details.needs_tls() || transport.is_tls() {
            return Ok(Some(Either::A(transport)));
        }

        let host = details
            .uri
            .authority()
            .map(ureq::http::uri::Authority::host)
            .ok_or(ureq::Error::Tls("HTTPS URI has no authority"))?;
        let server_name = ServerName::try_from(host.to_owned())
            .map_err(|_| ureq::Error::Tls("HTTPS URI has an invalid server name"))?;
        let provider = rustls::crypto::ring::default_provider();
        let algorithms = provider.signature_verification_algorithms;
        let config = ClientConfig::builder_with_provider(Arc::new(provider))
            .with_safe_default_protocol_versions()?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PinnedVerifier {
                expected: self.expected,
                presented: Arc::clone(&self.presented),
                algorithms,
            }))
            .with_no_client_auth();
        let connection = ClientConnection::new(Arc::new(config), server_name)?;
        let stream = rustls::StreamOwned {
            conn: connection,
            sock: TransportAdapter::new(transport.boxed(), details.current_time.clone()),
        };
        let buffers = LazyBuffers::new(
            details.config.input_buffer_size(),
            details.config.output_buffer_size(),
        );
        Ok(Some(Either::B(PinnedTlsTransport { buffers, stream })))
    }
}

struct PinnedTlsTransport {
    buffers: LazyBuffers,
    stream: rustls::StreamOwned<ClientConnection, TransportAdapter>,
}

impl Transport for PinnedTlsTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(
        &mut self,
        amount: usize,
        timeout: NextTimeout,
    ) -> std::result::Result<(), ureq::Error> {
        self.stream.get_mut().set_timeout(timeout);
        let output = self
            .buffers
            .output()
            .get(..amount)
            .ok_or(ureq::Error::ConnectionFailed)?;
        self.stream.write_all(output)?;
        Ok(())
    }

    fn await_input(&mut self, timeout: NextTimeout) -> std::result::Result<bool, ureq::Error> {
        self.stream.get_mut().set_timeout(timeout);
        let input = self.buffers.input_append_buf();
        let amount = self.stream.read(input)?;
        self.buffers.input_appended(amount);
        Ok(amount > 0)
    }

    fn is_open(&mut self) -> bool {
        self.stream.get_mut().get_mut().is_open()
    }

    fn is_tls(&self) -> bool {
        true
    }
}

impl fmt::Debug for PinnedTlsTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PinnedTlsTransport")
            .field("transport", &"<TLS stream>")
            .finish()
    }
}

#[derive(Debug)]
struct PinnedVerifier {
    expected: Fingerprint,
    presented: Arc<Mutex<Option<Fingerprint>>>,
    algorithms: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let digest: [u8; 32] = Sha256::digest(end_entity.as_ref()).into();
        let fingerprint = Fingerprint::from_sha256(digest);
        if let Ok(mut presented) = self.presented.lock() {
            *presented = Some(fingerprint);
        }
        if fingerprint == self.expected {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "scanner certificate does not match the pinned fingerprint".to_owned(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, certificate, signature, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, certificate, signature, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

fn response_headers(response: &ureq::http::Response<ureq::Body>) -> Vec<(String, String)> {
    response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.to_owned()))
        })
        .collect()
}

fn is_tls_error(error: &ureq::Error) -> bool {
    matches!(
        error,
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) | ureq::Error::ConnectionFailed
    )
}

fn map_ureq_error(operation: &str, error: ureq::Error) -> Error {
    let kind = match error {
        ureq::Error::Timeout(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => {
            ErrorKind::Unavailable
        },
        ureq::Error::BodyExceedsLimit(_) | ureq::Error::LargeResponseHeader(_, _) => {
            ErrorKind::Protocol
        },
        ureq::Error::BadUri(_) | ureq::Error::RequireHttpsOnly(_) => ErrorKind::InvalidInput,
        ureq::Error::Io(_) => ErrorKind::Io,
        _ => ErrorKind::Protocol,
    };
    Error::with_source(kind, format!("could not {operation}"), error)
}

fn peer_certificate(url: &Uri, timeout: Duration) -> Result<(Vec<u8>, Fingerprint)> {
    let host = url.host().ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidInput,
            "HTTPS scanner endpoint has no host",
        )
    })?;
    let port = url.port_u16().unwrap_or(443);
    let mut addresses = (host, port).to_socket_addrs().map_err(|error| {
        Error::with_source(
            ErrorKind::Unavailable,
            "could not resolve scanner host for TLS verification",
            error,
        )
    })?;
    let address = addresses.next().ok_or_else(|| {
        Error::new(
            ErrorKind::Unavailable,
            "scanner host resolved to no address",
        )
    })?;
    let mut socket = TcpStream::connect_timeout(&address, timeout).map_err(|error| {
        Error::with_source(
            ErrorKind::Unavailable,
            "could not connect to scanner for TLS verification",
            error,
        )
    })?;
    socket
        .set_read_timeout(Some(timeout))
        .and_then(|()| socket.set_write_timeout(Some(timeout)))
        .map_err(|error| {
            Error::with_source(
                ErrorKind::Io,
                "could not configure TLS verification socket",
                error,
            )
        })?;

    let provider = rustls::crypto::ring::default_provider();
    let algorithms = provider.signature_verification_algorithms;
    let config = ClientConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|error| {
            Error::with_source(
                ErrorKind::Protocol,
                "could not select TLS protocol versions",
                error,
            )
        })?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(ProbeVerifier { algorithms }))
        .with_no_client_auth();
    let server_name = ServerName::try_from(host.to_owned()).map_err(|error| {
        Error::with_source(
            ErrorKind::InvalidInput,
            "scanner host is not a valid TLS server name",
            error,
        )
    })?;
    let mut connection = ClientConnection::new(Arc::new(config), server_name).map_err(|error| {
        Error::with_source(
            ErrorKind::Protocol,
            "could not initialize TLS certificate probe",
            error,
        )
    })?;
    while connection.is_handshaking() {
        connection.complete_io(&mut socket).map_err(|error| {
            Error::with_source(
                ErrorKind::Unavailable,
                "TLS certificate probe failed",
                error,
            )
        })?;
    }
    let certificate = connection
        .peer_certificates()
        .and_then(|certificates| certificates.first())
        .map(|certificate| certificate.as_ref().to_vec())
        .ok_or_else(|| Error::new(ErrorKind::Protocol, "TLS scanner presented no certificate"))?;
    let digest: [u8; 32] = Sha256::digest(&certificate).into();
    Ok((certificate, Fingerprint::from_sha256(digest)))
}

#[derive(Debug)]
struct ProbeVerifier {
    algorithms: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for ProbeVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, certificate, signature, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, certificate, signature, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use rustls::client::danger::ServerCertVerifier;
    use rustls_pki_types::{CertificateDer, ServerName, UnixTime};
    use sha2::{Digest, Sha256};

    use crate::Fingerprint;

    use super::PinnedVerifier;

    #[test]
    fn pin_verifier_accepts_only_the_exact_certificate_bytes() {
        let certificate = CertificateDer::from(&b"scanner certificate"[..]);
        let digest: [u8; 32] = Sha256::digest(certificate.as_ref()).into();
        let verifier = PinnedVerifier {
            expected: Fingerprint::from_sha256(digest),
            presented: Arc::new(Mutex::new(None)),
            algorithms: rustls::crypto::ring::default_provider().signature_verification_algorithms,
        };
        let name = ServerName::try_from("scanner.local").expect("valid test name");
        let now = UnixTime::since_unix_epoch(Duration::ZERO);
        assert!(
            verifier
                .verify_server_cert(&certificate, &[], &name, &[], now)
                .is_ok()
        );

        let other = CertificateDer::from(&b"different certificate"[..]);
        assert!(
            verifier
                .verify_server_cert(&other, &[], &name, &[], now)
                .is_err()
        );
    }
}
