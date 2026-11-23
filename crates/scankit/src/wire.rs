// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::io::Read;

pub(crate) const MAX_CONTROL_BODY: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Method {
    Get,
    Post,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpCall {
    pub(crate) method: Method,
    pub(crate) target: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl HttpCall {
    pub(crate) fn get(target: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            target: target.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub(crate) fn post(target: impl Into<String>, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            method: Method::Post,
            target: target.into(),
            headers: vec![("content-type".to_owned(), content_type.to_owned())],
            body,
        }
    }

    pub(crate) fn delete(target: impl Into<String>) -> Self {
        Self {
            method: Method::Delete,
            target: target.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpReply {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl HttpReply {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

pub(crate) struct StreamReply {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Box<dyn Read + Send>,
}

impl StreamReply {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

impl fmt::Debug for StreamReply {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamReply")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .field("body", &"<stream>")
            .finish()
    }
}
