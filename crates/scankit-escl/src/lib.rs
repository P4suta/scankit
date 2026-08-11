// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The eSCL protocol as a sans-I/O state machine. This crate decides what to say to an
//! eSCL scanner and what its answers mean; it never says anything itself.
//!
//! eSCL (Mopria "AirScan") is the driverless protocol behind "it just works" scanning on
//! macOS and much of Linux. This crate will hold the whole of scankit's knowledge of it:
//!
//! - Interpretation of `ScannerCapabilities` XML into `scankit_core::Capabilities`.
//! - Generation of a `ScanSettings` request body from a validated `ScanTicket`.
//! - The job state machine: created, processing, page ready, completed, canceled, and
//!   every transition the specification and field behavior admit — expressed over the
//!   `HttpCall` / `HttpReply` value types from `scankit-core::exchange`.
//!
//! Every function here is a pure step: it takes protocol state and a reply, and returns
//! new state and the next call to make. The pump that owns sockets, retries, and time is
//! `scankit-backend-escl`; the split is `docs/adr/0003` and the reason this crate is
//! testable against recorded traffic alone (`docs/adr/0011`).
//!
//! # Invariants
//!
//! - No I/O, no clock, no socket type — `just purity` enforces it.
//! - The only third-party dependency this crate may ever take is `quick-xml`
//!   (`docs/adr/0003`); today it takes none.
//! - Where behavior and specification disagree, `sane-airscan` is the behavior oracle
//!   and the deviation is recorded with a pinned citation (`docs/adr/0012`).
//!
//! # Status
//!
//! Frozen at bootstrap: the shapes above are the day-one design, recorded in
//! `docs/adr/` before any of them is implemented. No code exists yet.
