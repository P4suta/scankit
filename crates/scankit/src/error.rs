// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::error::Error as StdError;
use std::fmt;

/// A SHA-256 fingerprint of a scanner's leaf certificate.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// Creates a fingerprint from a SHA-256 digest.
    #[must_use]
    pub const fn from_sha256(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Parses 64 hexadecimal digits, optionally prefixed by `sha256:`.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text
            .strip_prefix("sha256:")
            .or_else(|| text.strip_prefix("SHA256:"))
            .unwrap_or(text);
        if text.len() != 64 || !text.is_ascii() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "a certificate fingerprint must contain 64 hexadecimal digits",
            ));
        }

        let mut output = [0_u8; 32];
        let mut chunks = text.as_bytes().chunks_exact(2);
        for slot in &mut output {
            let Some(pair) = chunks.next() else {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "certificate fingerprint ended unexpectedly",
                ));
            };
            let Some(high) = pair.first().and_then(|value| hex_value(*value)) else {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "certificate fingerprint contains a non-hexadecimal digit",
                ));
            };
            let Some(low) = pair.get(1).and_then(|value| hex_value(*value)) else {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "certificate fingerprint contains a non-hexadecimal digit",
                ));
            };
            *slot = high
                .checked_mul(16)
                .and_then(|value| value.checked_add(low))
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::InvalidInput,
                        "certificate fingerprint digit overflowed",
                    )
                })?;
        }
        Ok(Self(output))
    }

    /// Returns the raw SHA-256 digest.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => byte.checked_sub(b'0'),
        b'a'..=b'f' => byte
            .checked_sub(b'a')
            .and_then(|value| value.checked_add(10)),
        b'A'..=b'F' => byte
            .checked_sub(b'A')
            .and_then(|value| value.checked_add(10)),
        _ => None,
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("sha256:")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// The result type returned by scankit.
pub type Result<T> = std::result::Result<T, Error>;

/// A stable, application-facing classification of a scanning failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The caller supplied an invalid value or used an object in the wrong state.
    InvalidInput,
    /// The scanner cannot honor an explicitly requested setting.
    Unsupported,
    /// A person must act at the scanner before work can continue.
    NeedsAttention,
    /// A TLS peer was not trusted or changed identity.
    UntrustedPeer,
    /// The scanner or network is temporarily unavailable.
    Unavailable,
    /// The device returned a response that the selected protocol cannot interpret.
    Protocol,
    /// A scan was canceled.
    Cancelled,
    /// Local I/O failed while communicating with the scanner.
    Io,
}

/// A condition that can normally be fixed by a person at the scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Intervention {
    /// The automatic document feeder has no paper.
    EmptyFeeder,
    /// Paper is jammed.
    PaperJam,
    /// A scanner cover is open.
    CoverOpen,
    /// The scanner is occupied by another job.
    Busy,
    /// The scanner is warming up.
    WarmingUp,
}

/// A scanning error with stable classification and private diagnostic detail.
pub struct Error {
    kind: ErrorKind,
    message: String,
    retryable: bool,
    intervention: Option<Intervention>,
    peer_fingerprint: Option<Fingerprint>,
    source: Option<Box<dyn StdError + Send + Sync>>,
}

impl Error {
    /// Returns the stable application-facing error class.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Reports whether retrying the operation can reasonably succeed.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        self.retryable
    }

    /// Returns the required human action, if this is a device intervention.
    #[must_use]
    pub fn intervention(&self) -> Option<Intervention> {
        self.intervention
    }

    /// Returns the certificate fingerprint presented by an untrusted TLS peer.
    #[must_use]
    pub fn peer_fingerprint(&self) -> Option<Fingerprint> {
        self.peer_fingerprint
    }

    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            retryable: false,
            intervention: None,
            peer_fingerprint: None,
            source: None,
        }
    }

    pub(crate) fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    pub(crate) fn with_intervention(
        message: impl Into<String>,
        intervention: Intervention,
    ) -> Self {
        Self {
            kind: ErrorKind::NeedsAttention,
            message: message.into(),
            retryable: true,
            intervention: Some(intervention),
            peer_fingerprint: None,
            source: None,
        }
    }

    pub(crate) fn untrusted(message: impl Into<String>, fingerprint: Fingerprint) -> Self {
        Self {
            kind: ErrorKind::UntrustedPeer,
            message: message.into(),
            retryable: false,
            intervention: None,
            peer_fingerprint: Some(fingerprint),
            source: None,
        }
    }

    pub(crate) fn with_source<E>(kind: ErrorKind, message: impl Into<String>, source: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self {
            kind,
            message: message.into(),
            retryable: matches!(kind, ErrorKind::Io | ErrorKind::Unavailable),
            intervention: None,
            peer_fingerprint: None,
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Error")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .field("retryable", &self.retryable)
            .field("intervention", &self.intervention)
            .field("peer_fingerprint", &self.peer_fingerprint)
            .finish_non_exhaustive()
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn StdError + 'static))
    }
}
