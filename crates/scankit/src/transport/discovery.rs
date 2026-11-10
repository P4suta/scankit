// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::ErrorKind as IoErrorKind;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use mdns_sd::{Receiver, ResolvedService, ServiceDaemon, ServiceEvent};

use crate::error::{Error, ErrorKind, Result};
use crate::protocol::ws_discovery;

const ESCL_SERVICE: &str = "_uscan._tcp.local.";
const ESCLS_SERVICE: &str = "_uscans._tcp.local.";
const WSD_MULTICAST: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), 3702);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

static PROBE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProtocolHint {
    Escl,
    Wsd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FoundEndpoint {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) protocol: ProtocolHint,
    pub(crate) secure: bool,
}

pub(crate) fn discover(timeout: Duration) -> Result<Vec<FoundEndpoint>> {
    if timeout.is_zero() {
        return Ok(Vec::new());
    }

    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "discovery timeout is too large"))?;
    let mdns = MdnsBrowse::start();
    let wsd = WsdBrowse::start();
    if mdns.is_none() && wsd.is_none() {
        return Err(Error::new(
            ErrorKind::Unavailable,
            "could not start either mDNS or WS-Discovery",
        ));
    }

    let mut found = Vec::new();
    while Instant::now() < deadline {
        if let Some(browser) = mdns.as_ref() {
            browser.drain(&mut found);
        }
        if let Some(browser) = wsd.as_ref() {
            browser.drain(&mut found)?;
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(remaining.min(POLL_INTERVAL));
    }
    if let Some(browser) = mdns.as_ref() {
        browser.drain(&mut found);
    }
    if let Some(browser) = wsd.as_ref() {
        browser.drain(&mut found)?;
    }

    found.sort_by(|left, right| {
        (&left.id, !left.secure, &left.url).cmp(&(&right.id, !right.secure, &right.url))
    });
    found.dedup_by(|left, right| {
        left.id == right.id && left.url == right.url && left.protocol == right.protocol
    });
    Ok(found)
}

struct MdnsBrowse {
    daemon: ServiceDaemon,
    receivers: [(bool, Receiver<ServiceEvent>); 2],
}

impl MdnsBrowse {
    fn start() -> Option<Self> {
        let daemon = ServiceDaemon::new().ok()?;
        let plain = daemon.browse(ESCL_SERVICE).ok()?;
        let secure = daemon.browse(ESCLS_SERVICE).ok()?;
        Some(Self {
            daemon,
            receivers: [(false, plain), (true, secure)],
        })
    }

    fn drain(&self, found: &mut Vec<FoundEndpoint>) {
        for (secure, receiver) in &self.receivers {
            while let Ok(event) = receiver.try_recv() {
                if let ServiceEvent::ServiceResolved(service) = event {
                    if let Some(endpoint) = mdns_endpoint(&service, *secure) {
                        found.push(endpoint);
                    }
                }
            }
        }
    }
}

impl Drop for MdnsBrowse {
    fn drop(&mut self) {
        let _ = self.daemon.stop_browse(ESCL_SERVICE);
        let _ = self.daemon.stop_browse(ESCLS_SERVICE);
        let _ = self.daemon.shutdown();
    }
}

fn mdns_endpoint(service: &ResolvedService, secure: bool) -> Option<FoundEndpoint> {
    let host = service.get_hostname().trim_end_matches('.');
    if host.is_empty() {
        return None;
    }
    let scheme = if secure { "https" } else { "http" };
    let resource = service
        .get_property_val_str("rs")
        .unwrap_or("eSCL")
        .trim_matches('/');
    let url = if resource.is_empty() {
        format!("{scheme}://{host}:{}", service.get_port())
    } else {
        format!("{scheme}://{host}:{}/{resource}", service.get_port())
    };
    let id = property(service, &["UUID", "uuid"])
        .map(normalize_id)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| normalize_id(service.get_fullname()));
    let name = property(service, &["ty", "product"])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or_else(|| instance_name(service.get_fullname()), str::to_owned);
    Some(FoundEndpoint {
        id,
        name,
        url,
        protocol: ProtocolHint::Escl,
        secure,
    })
}

fn property<'a>(service: &'a ResolvedService, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| service.get_property_val_str(key))
}

fn instance_name(fullname: &str) -> String {
    fullname
        .split("._")
        .next()
        .unwrap_or(fullname)
        .trim_end_matches('.')
        .to_owned()
}

struct WsdBrowse {
    socket: UdpSocket,
}

impl WsdBrowse {
    fn start() -> Option<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
        socket.set_nonblocking(true).ok()?;
        socket.set_multicast_ttl_v4(1).ok()?;
        let sequence = PROBE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let message_id = format!("{:08x}-{sequence:016x}", std::process::id());
        let probe = ws_discovery::probe(&message_id);
        socket.send_to(&probe, WSD_MULTICAST).ok()?;
        Some(Self { socket })
    }

    fn drain(&self, found: &mut Vec<FoundEndpoint>) -> Result<()> {
        let mut buffer = vec![0_u8; 65_507].into_boxed_slice();
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((length, _)) => {
                    let bytes = buffer.get(..length).ok_or_else(|| {
                        Error::new(
                            ErrorKind::Protocol,
                            "WS-Discovery socket returned an invalid datagram length",
                        )
                    })?;
                    for candidate in ws_discovery::parse_probe_matches(bytes)? {
                        let name = candidate.name.unwrap_or_else(|| candidate.id.clone());
                        for address in candidate.addresses {
                            let secure = address.starts_with("https://");
                            found.push(FoundEndpoint {
                                id: candidate.id.clone(),
                                name: name.clone(),
                                url: address,
                                protocol: ProtocolHint::Wsd,
                                secure,
                            });
                        }
                    }
                },
                Err(error) if error.kind() == IoErrorKind::WouldBlock => return Ok(()),
                Err(error) => {
                    return Err(Error::with_source(
                        ErrorKind::Io,
                        "could not receive WS-Discovery response",
                        error,
                    ));
                },
            }
        }
    }
}

fn normalize_id(value: &str) -> String {
    value
        .trim()
        .strip_prefix("urn:uuid:")
        .unwrap_or(value.trim())
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{instance_name, normalize_id};

    #[test]
    fn normalizes_discovery_identity() {
        assert_eq!(normalize_id("urn:uuid:ABC"), "abc");
        assert_eq!(
            instance_name("Office Scanner._uscan._tcp.local."),
            "Office Scanner"
        );
    }
}
