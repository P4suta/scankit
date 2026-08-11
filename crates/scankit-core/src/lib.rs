// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The shared vocabulary of the scankit scanner stack. Every other crate speaks in these
//! types; this crate depends on nothing at all.
//!
//! This crate exists so that a protocol model, a transport, a pump, and an application
//! can be written against one set of nouns without any of them depending on another's
//! implementation. It will hold, and nothing else will hold:
//!
//! - The `Backend`, `Scanner`, and `ScanJob` traits: native `async fn` in traits with
//!   static dispatch, runtime-agnostic. `&mut self` on every operation makes "one
//!   operation at a time per device" a fact the borrow checker enforces rather than a
//!   convention, and the GAT `type Job<'a>` borrows the scanner so a live job pins the
//!   device it runs on.
//! - `ScanTicket` and `Capabilities`: what the caller asks for and what the device
//!   offers. Capabilities are two layers — hand-rolled `ScannerFeature` bit flags with a
//!   `FLAGS` registry held consistent by a const assertion, and per-input-source tables,
//!   because "duplex" is a property of the ADF and not of the scanner.
//! - The flat error vocabulary: one closed `#[non_exhaustive]` enum, translated at each
//!   boundary rather than chained — no `source()` walk required to find out what
//!   happened. Failures a user can fix at the device — `AdfEmpty`, `AdfJam`,
//!   `CoverOpen` — are one `Intervention(InterventionReason)` class, distinct from
//!   protocol and transport failures, because "go press the button" and "give up" are
//!   different callers.
//! - The `exchange` module: `HttpCall`, `HttpReply`, and `Datagram` as pure value types.
//!   These are the words the sans-I/O protocol cores speak; no socket type appears in
//!   them.
//! - `ScannedPage`: `PageInfo` plus the bytes the device sent, delivered unmodified.
//!
//! # Invariants
//!
//! - No I/O, no clock, no thread, no third-party dependency — `just purity` and
//!   `just deps` enforce this mechanically.
//! - No trait object in the public surface; dispatch is static (`docs/adr/0004`).
//! - Errors are flat and translated, never wrapped (`docs/adr/0006`).
//!
//! # Status
//!
//! Frozen at bootstrap: the shapes above are the day-one design, recorded in
//! `docs/adr/` before any of them is implemented. No code exists yet; the first
//! implementation commit must conform to this document rather than amend it.
