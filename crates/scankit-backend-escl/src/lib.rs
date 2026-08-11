// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The eSCL backend: the pump that drives the sans-I/O protocol core in `scankit-escl`
//! over the transport in `scankit-http`, and implements the `scankit_core` traits.
//!
//! The protocol core answers "what should be said next"; this crate is the loop that
//! says it. Everything temporal lives here and nowhere below:
//!
//! - Retry policy: which failures are retried, how many times, with what backoff.
//! - Timeouts: how long a capability fetch, a job creation, or a page retrieval may
//!   take before it becomes an error.
//! - Polling: how often job status is asked for while a page is being produced.
//!
//! The core cannot name a clock (`just purity` refuses it), so this crate is the owner
//! of time by construction, not by convention (`docs/adr/0003`).
//!
//! # Invariants
//!
//! - No XML knowledge: if a byte of eSCL grammar appears here, it belongs in
//!   `scankit-escl`.
//! - Delivered pages are the device's bytes, unmodified (`docs/adr/0007`).
//! - Ticket/capability mismatches surface as structured errors before any request is
//!   sent (`docs/adr/0010`).
//!
//! # Status
//!
//! Frozen at bootstrap: the shapes above are the day-one design, recorded in
//! `docs/adr/` before any of them is implemented. No code exists yet.
