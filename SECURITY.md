# Security Policy

## Scope

scankit talks to devices on the local network and parses what they send. The realistic
threat is a hostile or broken device (or something impersonating one on the LAN)
reaching the parsers: capability XML, SOAP envelopes, WS-Discovery datagrams, and page
payloads are all untrusted input. A panic, an unbounded allocation, or non-termination
driven by any of them is a security issue here, because callers embed this stack in
services and desktop applications.

Specifically in scope:

- Panics or unbounded memory growth on any network-supplied bytes — XML, SOAP,
  datagrams, HTTP framing, or page data
- TLS trust bypass: any path that accepts a certificate the configured `TrustPolicy`
  does not admit ([ADR 0009](docs/adr/0009-tls-trust-is-explicit.md))
- A discovery response causing requests to be sent to an address the caller did not
  approve

Out of scope: a device that refuses to scan or scans incorrectly. That is an
interoperability report — use the issue template for it.

## Reporting

Report privately through GitHub's ["Report a vulnerability"][advisories] flow rather
than a public issue. Include the input (a capture if possible), the configuration, and
the observed behavior.

Expect an acknowledgement within seven days.

## Supported versions

While the project is pre-1.0, only the latest release receives fixes.

[advisories]: https://github.com/P4suta/scankit/security/advisories/new
