// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The workspace's own test harness. Dev-only, never published, and free of third-party
//! dependencies, so depending on it costs a test suite nothing.
//!
//! This crate exists because the stack is verified virtually first (`docs/adr/0011`):
//! the traits are native `async fn` with no runtime attached (`docs/adr/0004`), so the
//! tests need a way to drive a future without adopting tokio, and the pumps need a way
//! to replay recorded traffic without a scanner on the desk. It will hold:
//!
//! - `block_on` and `poll_n`: a minimal hand-rolled executor, enough to drive an AFIT
//!   future to completion or to a chosen suspension point deterministically.
//! - `ScriptedHttp`: an implementation of the transport surface that answers each
//!   `HttpCall` from a script and fails the test on any deviation.
//! - Cassettes: recorded request/reply exchanges from real devices, replayed byte for
//!   byte, so an interoperability fix is pinned by the traffic that motivated it.
//! - A hand-rolled `std::net` mock server for the tests that must cross a real socket.
//!
//! # Invariants
//!
//! - Dev-only: `publish = false`, and `just deps` (rule R5) rejects this crate in any
//!   normal dependency table, so it can never reach a shipped artifact.
//! - No third-party dependency: the harness that verifies the stack must not import a
//!   second stack to do it.
//!
//! # Status
//!
//! Frozen at bootstrap: the shapes above are the day-one design, recorded in
//! `docs/adr/` before any of them is implemented. No code exists yet.
