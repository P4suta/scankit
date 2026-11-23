// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{Error, ErrorKind, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextNode {
    pub(crate) path: Vec<String>,
    pub(crate) text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Document {
    nodes: Vec<TextNode>,
}

impl Document {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::from_reader(bytes);
        reader.config_mut().trim_text(true);
        reader.config_mut().check_end_names = true;

        let mut path = Vec::new();
        let mut nodes = Vec::new();
        loop {
            match reader.read_event() {
                Ok(Event::Start(element)) => {
                    path.push(local_name(element.name().as_ref())?);
                },
                Ok(Event::Text(text)) => {
                    let decoded = text.decode().map_err(|error| {
                        Error::with_source(
                            ErrorKind::Protocol,
                            "scanner returned text in an invalid XML encoding",
                            error,
                        )
                    })?;
                    let decoded = decoded.trim();
                    if !decoded.is_empty() {
                        nodes.push(TextNode {
                            path: path.clone(),
                            text: decoded.to_owned(),
                        });
                    }
                },
                Ok(Event::CData(text)) => {
                    let decoded = text.decode().map_err(|error| {
                        Error::with_source(
                            ErrorKind::Protocol,
                            "scanner returned CDATA in an invalid XML encoding",
                            error,
                        )
                    })?;
                    let decoded = decoded.trim();
                    if !decoded.is_empty() {
                        nodes.push(TextNode {
                            path: path.clone(),
                            text: decoded.to_owned(),
                        });
                    }
                },
                Ok(Event::End(_)) => {
                    let _ = path.pop();
                },
                Ok(Event::Eof) => break,
                Ok(
                    Event::Decl(_)
                    | Event::Empty(_)
                    | Event::Comment(_)
                    | Event::PI(_)
                    | Event::DocType(_)
                    | Event::GeneralRef(_),
                ) => {},
                Err(error) => {
                    return Err(Error::with_source(
                        ErrorKind::Protocol,
                        "scanner returned malformed XML",
                        error,
                    ));
                },
            }
        }

        if !path.is_empty() {
            return Err(Error::new(
                ErrorKind::Protocol,
                "scanner returned XML with an unclosed element",
            ));
        }
        Ok(Self { nodes })
    }

    pub(crate) fn nodes(&self) -> impl Iterator<Item = &TextNode> {
        self.nodes.iter()
    }

    pub(crate) fn first(&self, name: &str) -> Option<&str> {
        self.nodes
            .iter()
            .find(|node| node.path.last().is_some_and(|value| value == name))
            .map(|node| node.text.as_str())
    }

    pub(crate) fn contains_text(&self, name: &str, value: &str) -> bool {
        self.nodes.iter().any(|node| {
            node.path.last().is_some_and(|candidate| candidate == name)
                && node.text.eq_ignore_ascii_case(value)
        })
    }
}

fn local_name(name: &[u8]) -> Result<String> {
    let name = name.rsplit(|byte| *byte == b':').next().unwrap_or(name);
    std::str::from_utf8(name)
        .map(str::to_owned)
        .map_err(|error| {
            Error::with_source(
                ErrorKind::Protocol,
                "scanner returned a non-UTF-8 XML element name",
                error,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::Document;

    #[test]
    fn strips_namespace_prefixes_and_tracks_paths() {
        let document =
            Document::parse(br"<s:Envelope><scan:State>Processing</scan:State></s:Envelope>")
                .expect("valid document");
        assert_eq!(document.first("State"), Some("Processing"));
        let node = document.nodes().next().expect("one text node");
        assert_eq!(node.path, ["Envelope", "State"]);
    }
}
