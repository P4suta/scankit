# Roadmap

The architectural reset is complete: one product package, minimal synchronous public API, pure private protocol modules, real HTTP/TLS and discovery transports, eSCL/WSD job drivers, streaming pages, cancellation, and strict repository gates all compile and test together.

## Implemented foundation

- canonical per-source capabilities and exact `ScanOptions` resolution;
- eSCL capability, job creation, page retrieval, completion, status, and cancellation;
- WSD scanner elements, job creation, streaming multipart retrieval, status, and cancellation;
- mDNS for `_uscan`/`_uscans` plus WS-Discovery;
- platform TLS verification and explicit SHA-256 certificate pinning;
- one-job/one-page borrow discipline and opaque stable error classification;
- `skan list`, `skan info`, and `skan scan` as a public-API-only client;
- unit, protocol transcript, doctest, architecture, safety, and lint gates.

## Before 0.1

1. Grow the internal transcript suite from diverse scanner captures, especially WSD dialects and eSCL status variations.
2. Add deterministic loopback end-to-end fixtures for complete multi-page, feeder empty, jam, cancellation, timeout, oversized control body, and TLS pin mismatch.
3. Confirm discovery and one-page scans against representative hardware.
   Hardware is evidence and capture input, never a required CI dependency.
4. Audit protocol behavior against pinned sane-airscan sources where specifications are ambiguous, recording each compatibility quirk beside its test.
5. Run the API through small real consumers and remove anything that does not earn its SemVer cost before publishing 0.1.

## Release rule

Version remains `0.0.0` and release automation remains inert until the public surface has survived those consumer exercises.
A first release is a deliberate review of the root re-export list—not a milestone automatically triggered by implementation volume.

## Non-goals

SANE API compatibility, a C ABI, image decoding/processing, PDF assembly, USB legacy drivers, public backend injection, and vendor-proprietary protocols remain out of scope.
New protocol support should stay private unless it introduces genuinely new scanner-domain concepts.
