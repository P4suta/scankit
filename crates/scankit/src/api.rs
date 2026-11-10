// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::io::{self, Read};
use std::time::Duration;

use ureq::http::Uri;

use crate::driver::{Candidate, Driver, Job, PageStream};
use crate::error::{Error, ErrorKind, Fingerprint, Result};
use crate::model::{Capabilities, ColorMode, Resolution, ScanOptions, ScanPlan, Sides, Source};
use crate::transport::discovery::{self, FoundEndpoint, ProtocolHint};
use crate::transport::http::Options as TransportOptions;

/// Connection policy for a scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectOptions {
    connect_timeout: Duration,
    operation_timeout: Duration,
    fingerprint: Option<Fingerprint>,
}

impl ConnectOptions {
    /// Creates the default connection policy.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            operation_timeout: Duration::from_secs(60),
            fingerprint: None,
        }
    }

    /// Sets the limit for establishing a network connection.
    #[must_use]
    pub const fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Sets the limit for each scanner operation.
    #[must_use]
    pub const fn operation_timeout(mut self, timeout: Duration) -> Self {
        self.operation_timeout = timeout;
        self
    }

    /// Trusts exactly one SHA-256 leaf-certificate fingerprint.
    ///
    /// Pinning is valid only for an HTTPS endpoint.
    /// A changed certificate is reported as [`ErrorKind::UntrustedPeer`] and is never downgraded to HTTP.
    #[must_use]
    pub const fn pinned_certificate(mut self, fingerprint: Fingerprint) -> Self {
        self.fingerprint = Some(fingerprint);
        self
    }

    fn transport(self) -> Result<TransportOptions> {
        if self.connect_timeout.is_zero() || self.operation_timeout.is_zero() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "connection and operation timeouts must be greater than zero",
            ));
        }
        Ok(TransportOptions {
            connect_timeout: self.connect_timeout,
            operation_timeout: self.operation_timeout,
            pin: self.fingerprint,
        })
    }
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// A scanner discovered on the local network.
///
/// Multiple protocol endpoints with the same device identity are folded into one value.
/// Endpoint and protocol details remain private implementation choices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    id: String,
    name: String,
    candidates: Vec<Candidate>,
}

impl Device {
    /// Returns the stable identity advertised by the scanner.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the scanner's human-readable name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Connects using the default connection policy.
    pub fn connect(&self) -> Result<Scanner> {
        self.connect_with(ConnectOptions::new())
    }

    /// Connects using an explicit connection policy.
    pub fn connect_with(&self, options: ConnectOptions) -> Result<Scanner> {
        connect_device(self.clone(), options)
    }
}

/// Discovers eSCL and WSD scanners until `timeout` expires.
///
/// Results have deterministic ordering.
/// A device advertising both a secure and a plain endpoint is contacted only through the secure endpoint.
pub fn discover(timeout: Duration) -> Result<Vec<Device>> {
    let mut devices = Vec::<Device>::new();
    for endpoint in discovery::discover(timeout)? {
        merge_endpoint(&mut devices, endpoint);
    }
    for device in &mut devices {
        order_candidates(&mut device.candidates);
    }
    devices.sort_by(|left, right| (&left.name, &left.id).cmp(&(&right.name, &right.id)));
    Ok(devices)
}

fn merge_endpoint(devices: &mut Vec<Device>, endpoint: FoundEndpoint) {
    let candidate = Candidate {
        url: endpoint.url,
        protocol: endpoint.protocol,
        secure: endpoint.secure,
    };
    if let Some(device) = devices.iter_mut().find(|device| device.id == endpoint.id) {
        if !device.candidates.contains(&candidate) {
            device.candidates.push(candidate);
        }
        if device.name == device.id && endpoint.name != endpoint.id {
            device.name = endpoint.name;
        }
        return;
    }
    devices.push(Device {
        id: endpoint.id,
        name: endpoint.name,
        candidates: vec![candidate],
    });
}

fn order_candidates(candidates: &mut [Candidate]) {
    candidates.sort_by(|left, right| {
        (!left.secure, protocol_rank(left.protocol), &left.url).cmp(&(
            !right.secure,
            protocol_rank(right.protocol),
            &right.url,
        ))
    });
}

const fn protocol_rank(protocol: ProtocolHint) -> u8 {
    match protocol {
        ProtocolHint::Escl => 0,
        ProtocolHint::Wsd => 1,
    }
}

/// An established scanner session.
pub struct Scanner {
    device: Device,
    driver: Driver,
    capabilities: Capabilities,
    abandoned: Option<Job>,
}

impl Scanner {
    /// Connects directly to an eSCL root or WSD HTTP endpoint.
    ///
    /// The protocol is detected privately by trying capability exchanges; callers do not need to classify the URL.
    pub fn connect(endpoint: &str) -> Result<Self> {
        Self::connect_with(endpoint, ConnectOptions::new())
    }

    /// Connects directly with an explicit connection policy.
    pub fn connect_with(endpoint: &str, options: ConnectOptions) -> Result<Self> {
        let uri = endpoint.parse::<Uri>().map_err(|error| {
            Error::with_source(
                ErrorKind::InvalidInput,
                "scanner endpoint is not a valid absolute URL",
                error,
            )
        })?;
        let scheme = uri.scheme_str().ok_or_else(|| {
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
        if uri.authority().is_none() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "scanner endpoint has no URL authority",
            ));
        }
        let id = endpoint.to_ascii_lowercase();
        let name = uri.host().unwrap_or(endpoint).to_owned();
        let device = Device {
            id,
            name,
            candidates: [ProtocolHint::Escl, ProtocolHint::Wsd]
                .into_iter()
                .map(|protocol| Candidate {
                    url: endpoint.to_owned(),
                    protocol,
                    secure: scheme == "https",
                })
                .collect(),
        };
        connect_device(device, options)
    }

    /// Returns the discovered or caller-supplied device description.
    #[must_use]
    pub const fn device(&self) -> &Device {
        &self.device
    }

    /// Returns the normalized capabilities read while connecting.
    #[must_use]
    pub const fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Starts a scan using deterministic defaults.
    pub fn scan(&mut self) -> Result<Scan<'_>> {
        self.scan_with(&ScanOptions::new())
    }

    /// Resolves exact options and starts one scan job.
    pub fn scan_with(&mut self, options: &ScanOptions) -> Result<Scan<'_>> {
        self.cancel_abandoned()?;
        let plan = self.capabilities.resolve(options)?;
        let job = self.driver.start(&plan)?;
        Ok(Scan {
            scanner: self,
            job: Some(job),
            plan,
            next_number: 1,
            page: PageMarker::Ready,
            finished: false,
        })
    }

    fn cancel_abandoned(&mut self) -> Result<()> {
        if let Some(job) = self.abandoned.take() {
            self.driver.cancel(&job)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Scanner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Scanner")
            .field("device", &self.device)
            .field("capabilities", &self.capabilities)
            .field("has_abandoned_job", &self.abandoned.is_some())
            .finish_non_exhaustive()
    }
}

fn connect_device(device: Device, options: ConnectOptions) -> Result<Scanner> {
    let transport_options = options.transport()?;
    let secure_only = device.candidates.iter().any(|candidate| candidate.secure);
    let candidates = device
        .candidates
        .iter()
        .filter(|candidate| !secure_only || candidate.secure);
    let mut last_error = None;
    for candidate in candidates {
        let driver = match Driver::new(candidate, transport_options) {
            Ok(driver) => driver,
            Err(error) => {
                last_error = Some(error);
                continue;
            },
        };
        match driver.capabilities() {
            Ok(capabilities) => {
                return Ok(Scanner {
                    device,
                    driver,
                    capabilities,
                    abandoned: None,
                });
            },
            Err(error) if error.kind() == ErrorKind::UntrustedPeer => return Err(error),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        Error::new(
            ErrorKind::Unavailable,
            "device advertised no endpoint that connection policy permits",
        )
    }))
}

/// One active scan job borrowing its scanner session.
///
/// Only one job can exist per [`Scanner`].
/// Dropping a job before completion marks it for cancellation before the scanner starts another job.
///
/// ```compile_fail
/// # fn overlap(scanner: &mut scankit::Scanner) -> scankit::Result<()> {
/// let first = scanner.scan()?;
/// let second = scanner.scan()?; // `first` still holds the unique borrow
/// # drop((first, second));
/// # Ok(())
/// # }
/// ```
pub struct Scan<'scanner> {
    scanner: &'scanner mut Scanner,
    job: Option<Job>,
    plan: ScanPlan,
    next_number: u32,
    page: PageMarker,
    finished: bool,
}

impl Scan<'_> {
    /// Returns the exact settings selected for this job.
    #[must_use]
    pub const fn plan(&self) -> &ScanPlan {
        &self.plan
    }

    /// Streams the next page, or returns `None` after successful completion.
    ///
    /// A page must be read to EOF before another page can be requested.
    /// Dropping a partially read page makes the job unusable except for cancellation.
    pub fn next_page(&mut self) -> Result<Option<Page<'_>>> {
        match self.page {
            PageMarker::Ready | PageMarker::Consumed => {},
            PageMarker::Open => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "the current page is still open",
                ));
            },
            PageMarker::Abandoned => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "the previous page was dropped before it reached EOF; cancel this scan",
                ));
            },
        }
        if self.finished {
            return Ok(None);
        }
        let job = self
            .job
            .as_ref()
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "scan job is no longer active"))?;
        let Some(stream) = self.scanner.driver.next_page(job, self.plan.format())? else {
            self.finished = true;
            self.job = None;
            return Ok(None);
        };
        let number = self.next_number;
        self.next_number = self
            .next_number
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Protocol, "scanner returned too many pages"))?;
        self.page = PageMarker::Open;
        Ok(Some(Page::new(
            &mut self.page,
            stream,
            number,
            self.plan.source(),
            self.plan.sides(),
            self.plan.color_mode(),
            self.plan.resolution(),
        )))
    }

    /// Cancels the active job and consumes this handle.
    pub fn cancel(mut self) -> Result<()> {
        let result = if let Some(job) = self.job.take() {
            self.scanner.driver.cancel(&job)
        } else {
            Ok(())
        };
        self.finished = true;
        result
    }
}

impl fmt::Debug for Scan<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Scan")
            .field("plan", &self.plan)
            .field("next_number", &self.next_number)
            .field("page", &self.page)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl Drop for Scan<'_> {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            self.scanner.abandoned = Some(job);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageMarker {
    Ready,
    Open,
    Consumed,
    Abandoned,
}

/// A single scan page that implements [`Read`].
///
/// Image bytes flow directly from the scanner.
/// The page borrows its job so the type system prevents retrieving or cancelling around an open stream.
///
/// ```compile_fail
/// # fn overlap(scan: &mut scankit::Scan<'_>) -> scankit::Result<()> {
/// let page = scan.next_page()?;
/// let next = scan.next_page()?; // `page` still holds the page-state borrow
/// # drop((page, next));
/// # Ok(())
/// # }
/// ```
pub struct Page<'scan> {
    marker: &'scan mut PageMarker,
    body: Box<dyn Read + Send>,
    number: u32,
    format: String,
    source: Source,
    sides: Sides,
    color_mode: ColorMode,
    resolution: Resolution,
}

impl<'scan> Page<'scan> {
    fn new(
        marker: &'scan mut PageMarker,
        stream: PageStream,
        number: u32,
        source: Source,
        sides: Sides,
        color_mode: ColorMode,
        resolution: Resolution,
    ) -> Self {
        Self {
            marker,
            body: stream.body,
            number,
            format: stream.format,
            source,
            sides,
            color_mode,
            resolution,
        }
    }

    /// Returns this page's one-based sequence number.
    #[must_use]
    pub const fn number(&self) -> u32 {
        self.number
    }

    /// Returns the MIME media type reported for the bytes.
    #[must_use]
    pub fn format(&self) -> &str {
        &self.format
    }

    /// Returns a conventional file extension for the page's media type.
    #[must_use]
    pub fn suggested_extension(&self) -> Option<&'static str> {
        match self.format.to_ascii_lowercase().as_str() {
            "image/jpeg" | "image/jpg" => Some("jpg"),
            "image/png" => Some("png"),
            "image/tiff" => Some("tiff"),
            "application/pdf" => Some("pdf"),
            _ => None,
        }
    }

    /// Returns the physical input used for this page.
    #[must_use]
    pub const fn source(&self) -> Source {
        self.source
    }

    /// Returns the side mode used for this page.
    #[must_use]
    pub const fn sides(&self) -> Sides {
        self.sides
    }

    /// Returns the color mode used for this page.
    #[must_use]
    pub const fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    /// Returns the resolution used for this page.
    #[must_use]
    pub const fn resolution(&self) -> Resolution {
        self.resolution
    }
}

impl Read for Page<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self.body.read(buffer) {
            Ok(0) if !buffer.is_empty() => {
                *self.marker = PageMarker::Consumed;
                Ok(0)
            },
            Ok(read) => Ok(read),
            Err(error) => {
                *self.marker = PageMarker::Abandoned;
                Err(error)
            },
        }
    }
}

impl fmt::Debug for Page<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Page")
            .field("number", &self.number)
            .field("format", &self.format)
            .field("source", &self.source)
            .field("sides", &self.sides)
            .field("color_mode", &self.color_mode)
            .field("resolution", &self.resolution)
            .field("body", &"<stream>")
            .finish()
    }
}

impl Drop for Page<'_> {
    fn drop(&mut self) {
        if *self.marker == PageMarker::Open {
            *self.marker = PageMarker::Abandoned;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use crate::driver::PageStream;
    use crate::{ColorMode, Resolution, Sides, Source};

    use super::{Page, PageMarker, Scanner};

    const CAPABILITIES: &str = r#"
      <scan:ScannerCapabilities xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03"
          xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm">
        <scan:Platen><scan:PlatenInputCaps>
          <scan:MaxWidth>2550</scan:MaxWidth><scan:MaxHeight>3507</scan:MaxHeight>
          <scan:ColorMode>RGB24</scan:ColorMode>
          <pwg:DocumentFormat>image/jpeg</pwg:DocumentFormat>
          <scan:XResolution>300</scan:XResolution>
        </scan:PlatenInputCaps></scan:Platen>
      </scan:ScannerCapabilities>
    "#;

    #[test]
    fn page_must_reach_eof_to_be_consumed() {
        let mut marker = PageMarker::Open;
        {
            let stream = PageStream {
                format: "image/jpeg".to_owned(),
                body: Box::new(Cursor::new(b"page")),
            };
            let mut page = Page::new(
                &mut marker,
                stream,
                1,
                Source::Flatbed,
                Sides::One,
                ColorMode::Color,
                Resolution::dpi(300),
            );
            let mut output = Vec::new();
            assert!(page.read_to_end(&mut output).is_ok());
            assert_eq!(output, b"page");
        }
        assert_eq!(marker, PageMarker::Consumed);
    }

    #[test]
    fn dropping_partial_page_marks_job_abandoned() {
        let mut marker = PageMarker::Open;
        {
            let stream = PageStream {
                format: "image/jpeg".to_owned(),
                body: Box::new(Cursor::new(b"page")),
            };
            let _page = Page::new(
                &mut marker,
                stream,
                1,
                Source::Flatbed,
                Sides::One,
                ColorMode::Color,
                Resolution::dpi(300),
            );
        }
        assert_eq!(marker, PageMarker::Abandoned);
    }

    #[test]
    fn public_api_completes_a_streaming_escl_job_over_loopback() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback fixture");
        let address = listener.local_addr().expect("fixture address");
        let server = thread::spawn(move || serve_escl_fixture(&listener));

        let endpoint = format!("http://{address}/eSCL");
        let mut scanner = Scanner::connect(&endpoint).expect("connect fixture scanner");
        assert_eq!(
            scanner.capabilities().sources().collect::<Vec<_>>(),
            [Source::Flatbed]
        );
        let mut scan = scanner.scan().expect("start fixture job");
        let plan = scan.plan().clone();
        assert_eq!(plan.resolution(), Resolution::dpi(300));

        let mut bytes = Vec::new();
        let Some(mut page) = scan.next_page().expect("retrieve fixture page") else {
            panic!("fixture omitted its page");
        };
        assert_eq!(page.number(), 1);
        assert_eq!(page.format(), "image/jpeg");
        assert!(page.read_to_end(&mut bytes).is_ok());
        drop(page);
        assert_eq!(bytes, b"jpeg page bytes");
        assert!(scan.next_page().expect("complete fixture job").is_none());
        drop(scan);

        let joined = server.join();
        assert!(
            matches!(&joined, Ok(Ok(()))),
            "fixture server failed: {joined:?}"
        );
    }

    fn serve_escl_fixture(listener: &TcpListener) -> io::Result<()> {
        for step in 0_u8..4 {
            let (mut stream, _) = listener.accept()?;
            read_request(&mut stream)?;
            match step {
                0 => write_response(
                    &mut stream,
                    "200 OK",
                    "text/xml",
                    "",
                    CAPABILITIES.as_bytes(),
                )?,
                1 => write_response(
                    &mut stream,
                    "201 Created",
                    "text/xml",
                    "Location: /eSCL/ScanJobs/1\r\n",
                    &[],
                )?,
                2 => write_response(&mut stream, "200 OK", "image/jpeg", "", b"jpeg page bytes")?,
                3 => write_response(&mut stream, "404 Not Found", "text/xml", "", &[])?,
                _ => return Err(io::Error::other("fixture received an impossible step")),
            }
        }
        Ok(())
    }

    fn read_request(stream: &mut TcpStream) -> io::Result<()> {
        let mut request = Vec::new();
        let header_length = loop {
            if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                break position
                    .checked_add(4)
                    .ok_or_else(|| io::Error::other("fixture request overflow"))?;
            }
            let mut chunk = [0_u8; 4 * 1024];
            let read = stream.read(&mut chunk)?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "fixture request ended before its headers",
                ));
            }
            let bytes = chunk
                .get(..read)
                .ok_or_else(|| io::Error::other("fixture reader returned an invalid length"))?;
            request.extend_from_slice(bytes);
        };

        let header_bytes = request
            .get(..header_length)
            .ok_or_else(|| io::Error::other("fixture header length is inconsistent"))?;
        let headers = String::from_utf8_lossy(header_bytes);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        let total = header_length
            .checked_add(content_length)
            .ok_or_else(|| io::Error::other("fixture request overflow"))?;
        while request.len() < total {
            let mut chunk = [0_u8; 4 * 1024];
            let read = stream.read(&mut chunk)?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "fixture request body ended early",
                ));
            }
            let bytes = chunk
                .get(..read)
                .ok_or_else(|| io::Error::other("fixture reader returned an invalid length"))?;
            request.extend_from_slice(bytes);
        }
        Ok(())
    }

    fn write_response(
        stream: &mut TcpStream,
        status: &str,
        content_type: &str,
        extra_headers: &str,
        body: &[u8],
    ) -> io::Result<()> {
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n",
            body.len(),
        )?;
        stream.write_all(body)
    }
}
