// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The WSD scan service as sans-I/O models: the WS-Scan SOAP vocabulary and the
//! WS-Discovery datagram codec. This crate decides what the messages mean; it never
//! sends one.
//!
//! WSD ("Web Services on Devices") is the driverless protocol Windows speaks to network
//! scanners, and the one many office devices implement first. This crate will hold:
//!
//! - The WS-Scan SOAP model: `GetScannerElements`, `CreateScanJob`, `RetrieveImage`,
//!   and the fault vocabulary, as typed request and reply values rendered to and parsed
//!   from XML — expressed over `HttpCall` / `HttpReply` from `scankit_core::exchange`.
//! - The WS-Discovery codec: `Probe`, `ProbeMatch`, `Resolve`, `Hello`, and `Bye` as
//!   pure functions between typed values and `Datagram` payloads. The socket that
//!   multicasts them lives in `scankit-discover`.
//!
//! # Invariants
//!
//! - No I/O, no clock, no socket type — `just purity` enforces it.
//! - The only third-party dependency this crate may ever take is `quick-xml`
//!   (`docs/adr/0003`); today it takes none.
//! - Microsoft's WSD scan documentation is partially decaying (`docs/M0-ground-truth.md`
//!   records which pages survive); every rule implemented here cites an archived copy,
//!   and `sane-airscan` is the behavior oracle where the text is gone
//!   (`docs/adr/0012`).
//!
//! # Status
//!
//! Frozen at bootstrap: the shapes above are the day-one design, recorded in
//! `docs/adr/` before any of them is implemented. No code exists yet.
