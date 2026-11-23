# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The `scan-kit` product package with library target `scankit` and `skan` command.
- Minimal synchronous discovery, connection, capability, scan-job, and streaming-page API with exact option resolution and opaque stable errors.
- Private pure eSCL, WSD, WS-Discovery, and XML layers; HTTP/TLS, mDNS, and UDP transports; eSCL/WSD job drivers; streaming WSD multipart extraction.
- Platform TLS verification, explicit SHA-256 leaf-certificate pinning, secure-endpoint preference without HTTP downgrade, bounded control replies, and disabled redirects.
- Module-level architecture gates, strict parser/safety lints, protocol transcript tests, lifecycle tests, doctests, and the single-product-package workspace register.
- ADR 0016, superseding the unpublished thirteen-crate bootstrap with private module boundaries and a deliberately narrow SemVer surface.
- `docs/M0-ground-truth.md`: the M0 measurements — incumbent SLOC (sane-backends,
  sane-airscan), dependency-tree costs of the candidate crates, crates.io name availability, and specification URL liveness — that the frozen design cites.
