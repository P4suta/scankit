// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::{Error, ErrorKind, Result};
use crate::protocol::xml::Document;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeMatch {
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    pub(crate) addresses: Vec<String>,
}

pub(crate) fn probe(message_id: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><s:Envelope xmlns:s=\"http://www.w3.org/2003/05/soap-envelope\" xmlns:a=\"http://schemas.xmlsoap.org/ws/2004/08/addressing\" xmlns:d=\"http://schemas.xmlsoap.org/ws/2005/04/discovery\" xmlns:dn=\"http://www.onvif.org/ver10/network/wsdl\"><s:Header><a:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</a:Action><a:MessageID>urn:uuid:{message_id}</a:MessageID><a:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</a:To></s:Header><s:Body><d:Probe/></s:Body></s:Envelope>"
    )
    .into_bytes()
}

pub(crate) fn parse_probe_matches(bytes: &[u8]) -> Result<Vec<ProbeMatch>> {
    let document = Document::parse(bytes)?;
    let addresses = document
        .nodes()
        .filter(|node| {
            node.path
                .last()
                .is_some_and(|name| name.eq_ignore_ascii_case("XAddrs"))
        })
        .flat_map(|node| node.text.split_ascii_whitespace())
        .filter(|address| address.starts_with("http://") || address.starts_with("https://"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Ok(Vec::new());
    }

    let id = document.first("Address").map_or_else(
        || addresses.first().cloned().unwrap_or_default(),
        normalize_id,
    );
    if id.is_empty() {
        return Err(Error::new(
            ErrorKind::Protocol,
            "WS-Discovery response has neither an endpoint identifier nor an address",
        ));
    }
    let name = document
        .first("FriendlyName")
        .or_else(|| document.first("ModelName"))
        .map(str::to_owned);
    Ok(vec![ProbeMatch {
        id,
        name,
        addresses,
    }])
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
    use super::{parse_probe_matches, probe};

    #[test]
    fn probe_has_a_caller_supplied_identity() {
        let probe = String::from_utf8(probe("1234")).expect("probe is UTF-8");
        assert!(probe.contains("urn:uuid:1234"));
    }

    #[test]
    fn reads_an_endpoint_without_exposing_soap() {
        let response = br#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery"><s:Body><d:ProbeMatches><d:ProbeMatch><a:EndpointReference><a:Address>urn:uuid:ABC</a:Address></a:EndpointReference><d:XAddrs>http://scanner/WSDScanner</d:XAddrs></d:ProbeMatch></d:ProbeMatches></s:Body></s:Envelope>"#;
        let matches = parse_probe_matches(response).expect("valid response");
        let found = matches.first().expect("one match");
        assert_eq!(found.id, "abc");
        assert_eq!(found.addresses, ["http://scanner/WSDScanner"]);
    }
}
