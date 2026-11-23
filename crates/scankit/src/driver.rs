// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::io::{self, BufRead, BufReader, Read};

use ureq::http::Uri;

use crate::error::{Error, ErrorKind, Result};
use crate::model::{Capabilities, ScanPlan};
use crate::protocol::{escl, wsd};
use crate::transport::discovery::ProtocolHint;
use crate::transport::http::{HttpTransport, Options as TransportOptions};
use crate::wire::{HttpReply, MAX_CONTROL_BODY, StreamReply};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) url: String,
    pub(crate) protocol: ProtocolHint,
    pub(crate) secure: bool,
}

#[derive(Debug)]
pub(crate) struct Driver {
    endpoint: String,
    protocol: ProtocolHint,
    transport: HttpTransport,
}

impl Driver {
    pub(crate) fn new(candidate: &Candidate, options: TransportOptions) -> Result<Self> {
        Ok(Self {
            endpoint: candidate.url.clone(),
            protocol: candidate.protocol,
            transport: HttpTransport::new(options, candidate.secure)?,
        })
    }

    pub(crate) fn capabilities(&self) -> Result<Capabilities> {
        match self.protocol {
            ProtocolHint::Escl => {
                let reply = self
                    .transport
                    .control(&escl::capabilities_call(&self.endpoint))?;
                escl::parse_capabilities(&reply)
            },
            ProtocolHint::Wsd => {
                let reply = self
                    .transport
                    .control(&wsd::capabilities_call(&self.endpoint))?;
                wsd::parse_capabilities(&reply)
            },
        }
    }

    pub(crate) fn start(&self, plan: &ScanPlan) -> Result<Job> {
        match self.protocol {
            ProtocolHint::Escl => {
                let call = escl::create_job_call(&self.endpoint, plan)?;
                let job_collection = call.target.clone();
                let reply = self.transport.control(&call)?;
                let location = escl::parse_job_location(&reply)?;
                let job_url = resolve_location(&job_collection, &location)?;
                Ok(Job::Escl { job_url })
            },
            ProtocolHint::Wsd => {
                let reply = self
                    .transport
                    .control(&wsd::create_job_call(&self.endpoint, plan)?)?;
                let job_id = wsd::parse_job_id(&reply)?;
                Ok(Job::Wsd {
                    endpoint: self.endpoint.clone(),
                    job_id,
                })
            },
        }
    }

    pub(crate) fn next_page(&self, job: &Job, fallback_format: &str) -> Result<Option<PageStream>> {
        match job {
            Job::Escl { job_url } => {
                let reply = self.transport.stream(&escl::next_document_call(job_url))?;
                page_from_escl(reply, fallback_format)
            },
            Job::Wsd { endpoint, job_id } => {
                let reply = self
                    .transport
                    .stream(&wsd::retrieve_image_call(endpoint, job_id))?;
                page_from_wsd(reply, fallback_format)
            },
        }
    }

    pub(crate) fn cancel(&self, job: &Job) -> Result<()> {
        let reply = match job {
            Job::Escl { job_url } => self.transport.control(&escl::cancel_call(job_url))?,
            Job::Wsd { endpoint, job_id } => self
                .transport
                .control(&wsd::cancel_call(endpoint, job_id))?,
        };
        if (200..300).contains(&reply.status) || matches!(reply.status, 404 | 410) {
            return Ok(());
        }
        let protocol_error = match job {
            Job::Escl { .. } => escl::status_error(&reply),
            Job::Wsd { .. } => wsd::status_error(&reply),
        };
        Err(protocol_error.unwrap_or_else(|| {
            Error::new(
                ErrorKind::Protocol,
                format!("scanner refused to cancel the job: HTTP {}", reply.status),
            )
        }))
    }
}

#[derive(Debug)]
pub(crate) enum Job {
    Escl { job_url: String },
    Wsd { endpoint: String, job_id: String },
}

pub(crate) struct PageStream {
    pub(crate) format: String,
    pub(crate) body: Box<dyn Read + Send>,
}

impl fmt::Debug for PageStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PageStream")
            .field("format", &self.format)
            .field("body", &"<stream>")
            .finish()
    }
}

fn page_from_escl(reply: StreamReply, fallback_format: &str) -> Result<Option<PageStream>> {
    if reply.status == 204 || escl::response_ends_job(reply.status) {
        return Ok(None);
    }
    if !(200..300).contains(&reply.status) {
        let reply = read_control_reply(reply)?;
        return Err(escl::status_error(&reply).unwrap_or_else(|| {
            Error::new(
                ErrorKind::Protocol,
                format!("could not retrieve eSCL page: HTTP {}", reply.status),
            )
        }));
    }
    let format = media_type(reply.header("content-type")).unwrap_or(fallback_format);
    Ok(Some(PageStream {
        format: format.to_owned(),
        body: reply.body,
    }))
}

fn page_from_wsd(reply: StreamReply, fallback_format: &str) -> Result<Option<PageStream>> {
    if matches!(reply.status, 404 | 409 | 410) {
        return Ok(None);
    }
    if !(200..300).contains(&reply.status) {
        let reply = read_control_reply(reply)?;
        return Err(wsd::status_error(&reply).unwrap_or_else(|| {
            Error::new(
                ErrorKind::Protocol,
                format!("could not retrieve WSD page: HTTP {}", reply.status),
            )
        }));
    }

    let content_type = reply.header("content-type").unwrap_or("").to_owned();
    if let Some(boundary) = multipart_boundary(&content_type) {
        let (format, body) = open_binary_multipart(reply.body, &boundary, fallback_format)?;
        return Ok(Some(PageStream { format, body }));
    }
    if let Some(format) = media_type(Some(&content_type)).filter(|value| !is_soap(value)) {
        return Ok(Some(PageStream {
            format: format.to_owned(),
            body: reply.body,
        }));
    }

    let reply = read_control_reply(reply)?;
    if wsd::response_ends_job(&reply) {
        return Ok(None);
    }
    Err(wsd::status_error(&reply).unwrap_or_else(|| {
        Error::new(
            ErrorKind::Protocol,
            "WSD scanner returned control XML where image data was expected",
        )
    }))
}

fn read_control_reply(mut reply: StreamReply) -> Result<HttpReply> {
    let mut body = Vec::new();
    reply
        .body
        .by_ref()
        .take(MAX_CONTROL_BODY.saturating_add(1))
        .read_to_end(&mut body)
        .map_err(|error| {
            Error::with_source(
                ErrorKind::Io,
                "could not read scanner control response",
                error,
            )
        })?;
    let length = u64::try_from(body.len()).map_err(|error| {
        Error::with_source(ErrorKind::Protocol, "scanner response is too large", error)
    })?;
    if length > MAX_CONTROL_BODY {
        return Err(Error::new(
            ErrorKind::Protocol,
            "scanner control response exceeded the size limit",
        ));
    }
    Ok(HttpReply {
        status: reply.status,
        headers: reply.headers,
        body,
    })
}

fn resolve_location(endpoint: &str, location: &str) -> Result<String> {
    let base = endpoint.parse::<Uri>().map_err(|error| {
        Error::with_source(
            ErrorKind::Protocol,
            "scan-job collection is not a valid absolute URL",
            error,
        )
    })?;
    let scheme = base
        .scheme_str()
        .ok_or_else(|| Error::new(ErrorKind::Protocol, "scan-job collection has no URL scheme"))?;
    let authority = base.authority().ok_or_else(|| {
        Error::new(
            ErrorKind::Protocol,
            "scan-job collection has no URL authority",
        )
    })?;

    if let Ok(absolute) = location.parse::<Uri>() {
        if absolute.scheme().is_some() || absolute.authority().is_some() {
            if absolute.scheme_str() == Some(scheme) && absolute.authority() == Some(authority) {
                return Ok(location.to_owned());
            }
            return Err(Error::new(
                ErrorKind::Protocol,
                "scanner returned a cross-origin scan-job location",
            ));
        }
    }

    let path = if location.starts_with('/') {
        location.to_owned()
    } else {
        let directory = base
            .path()
            .rsplit_once('/')
            .map_or("", |(directory, _)| directory);
        format!("{directory}/{location}")
    };
    let resolved = format!("{scheme}://{authority}{path}");
    resolved.parse::<Uri>().map(|_| resolved).map_err(|error| {
        Error::with_source(
            ErrorKind::Protocol,
            "scanner returned an invalid scan-job location",
            error,
        )
    })
}

fn media_type(content_type: Option<&str>) -> Option<&str> {
    content_type?
        .split(';')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn is_soap(media_type: &str) -> bool {
    media_type.eq_ignore_ascii_case("application/soap+xml")
        || media_type.eq_ignore_ascii_case("application/xop+xml")
        || media_type.eq_ignore_ascii_case("text/xml")
}

fn multipart_boundary(content_type: &str) -> Option<String> {
    if !content_type
        .split(';')
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("multipart/related"))
    {
        return None;
    }
    content_type.split(';').skip(1).find_map(|parameter| {
        let (name, value) = parameter.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("boundary")
            .then(|| value.trim().trim_matches('"').to_owned())
            .filter(|value| !value.is_empty())
    })
}

fn open_binary_multipart(
    body: Box<dyn Read + Send>,
    boundary: &str,
    fallback_format: &str,
) -> Result<(String, Box<dyn Read + Send>)> {
    let marker = format!("--{boundary}");
    let mut reader = BufReader::new(body);
    let mut line = String::new();
    read_limited_line(&mut reader, &mut line)?;
    if line.trim_end() != marker {
        return Err(Error::new(
            ErrorKind::Protocol,
            "WSD multipart response did not begin with its declared boundary",
        ));
    }

    loop {
        let headers = read_part_headers(&mut reader)?;
        let content_type = header(&headers, "content-type").unwrap_or(fallback_format);
        let format = media_type(Some(content_type)).unwrap_or(fallback_format);
        if !is_soap(format) {
            let delimiter = format!("\r\n--{boundary}").into_bytes();
            let stream = BoundaryReader::new(reader, delimiter);
            return Ok((format.to_owned(), Box::new(stream)));
        }

        let mut consumed = 0_u64;
        loop {
            line.clear();
            let read = read_limited_line(&mut reader, &mut line)?;
            if read == 0 {
                return Err(Error::new(
                    ErrorKind::Protocol,
                    "WSD multipart response ended before its image part",
                ));
            }
            consumed = consumed.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
            if consumed > MAX_CONTROL_BODY {
                return Err(Error::new(
                    ErrorKind::Protocol,
                    "WSD multipart control part exceeded the size limit",
                ));
            }
            if line.trim_end() == marker {
                break;
            }
            if line.trim_end() == format!("{marker}--") {
                return Err(Error::new(
                    ErrorKind::Protocol,
                    "WSD multipart response contained no image part",
                ));
            }
        }
    }
}

fn read_part_headers(
    reader: &mut BufReader<Box<dyn Read + Send>>,
) -> Result<Vec<(String, String)>> {
    let mut headers = Vec::new();
    let mut total = 0_usize;
    loop {
        let mut line = String::new();
        let read = read_limited_line(reader, &mut line)?;
        if read == 0 {
            return Err(Error::new(
                ErrorKind::Protocol,
                "WSD multipart response ended inside its headers",
            ));
        }
        total = total.saturating_add(read);
        if total > 64 * 1024 {
            return Err(Error::new(
                ErrorKind::Protocol,
                "WSD multipart headers exceeded the size limit",
            ));
        }
        if line == "\r\n" || line == "\n" {
            return Ok(headers);
        }
        let (name, value) = line.split_once(':').ok_or_else(|| {
            Error::new(
                ErrorKind::Protocol,
                "WSD multipart response has an invalid header",
            )
        })?;
        headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }
}

fn read_limited_line(
    reader: &mut BufReader<Box<dyn Read + Send>>,
    line: &mut String,
) -> Result<usize> {
    reader.read_line(line).map_err(|error| {
        Error::with_source(
            ErrorKind::Io,
            "could not read WSD multipart response",
            error,
        )
    })
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

struct BoundaryReader {
    reader: BufReader<Box<dyn Read + Send>>,
    delimiter: Vec<u8>,
    buffer: Vec<u8>,
    finished: bool,
}

impl BoundaryReader {
    fn new(reader: BufReader<Box<dyn Read + Send>>, delimiter: Vec<u8>) -> Self {
        Self {
            reader,
            delimiter,
            buffer: Vec::new(),
            finished: false,
        }
    }
}

impl Read for BoundaryReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() || self.finished {
            return Ok(0);
        }
        loop {
            if let Some(position) = find_bytes(&self.buffer, &self.delimiter) {
                if position == 0 {
                    self.buffer.clear();
                    self.finished = true;
                    return Ok(0);
                }
                let length = position.min(output.len());
                return self.copy_prefix(output, length);
            }

            let retained = self.delimiter.len().saturating_sub(1);
            if self.buffer.len() > retained {
                let safe = self.buffer.len().saturating_sub(retained);
                let length = safe.min(output.len());
                return self.copy_prefix(output, length);
            }

            let mut chunk = [0_u8; 8 * 1024];
            let read = self.reader.read(&mut chunk)?;
            if read == 0 {
                let length = self.buffer.len().min(output.len());
                self.copy_prefix(output, length)?;
                if length == 0 {
                    self.finished = true;
                }
                return Ok(length);
            }
            let bytes = chunk.get(..read).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "reader returned an invalid length",
                )
            })?;
            self.buffer.extend_from_slice(bytes);
        }
    }
}

impl BoundaryReader {
    fn copy_prefix(&mut self, output: &mut [u8], length: usize) -> io::Result<usize> {
        if length > output.len() || length > self.buffer.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "multipart boundary buffer length is inconsistent",
            ));
        }
        for (target, source) in output.iter_mut().zip(self.buffer.iter()).take(length) {
            *target = *source;
        }
        self.buffer.drain(..length);
        Ok(length)
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor, Read};

    use super::{BoundaryReader, multipart_boundary};

    #[test]
    fn parses_quoted_multipart_boundary() {
        assert_eq!(
            multipart_boundary("multipart/related; type=application/xop+xml; boundary=\"abc\""),
            Some("abc".to_owned()),
        );
    }

    #[test]
    fn boundary_reader_does_not_leak_mime_trailer() {
        let input: Box<dyn Read + Send> = Box::new(Cursor::new(b"image-data\r\n--abc--\r\n"));
        let mut reader = BoundaryReader::new(BufReader::new(input), b"\r\n--abc".to_vec());
        let mut output = Vec::new();
        let result = reader.read_to_end(&mut output);
        assert!(result.is_ok());
        assert_eq!(output, b"image-data");
    }
}
