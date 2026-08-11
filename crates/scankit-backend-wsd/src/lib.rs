// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The WSD backend: the pump that drives the sans-I/O WS-Scan model in `scankit-wsd`
//! over the transport in `scankit-http`, and implements the `scankit_core` traits.
//!
//! Structurally the twin of `scankit-backend-escl`, and deliberately so: the model
//! crate answers "what should be said next", and this crate owns everything temporal —
//! retries, timeouts, and the subscription or polling cadence WS-Scan job monitoring
//! needs. What differs from eSCL is the grammar and the fault model, and both of those
//! live in `scankit-wsd`, not here (`docs/adr/0003`).
//!
//! # Invariants
//!
//! - No SOAP knowledge: if an envelope is built or parsed here, it belongs in
//!   `scankit-wsd`.
//! - Delivered pages are the device's bytes, unmodified (`docs/adr/0007`).
//! - Ticket/capability mismatches surface as structured errors before any request is
//!   sent (`docs/adr/0010`).
//!
//! # Status
//!
//! Frozen at bootstrap: the shapes above are the day-one design, recorded in
//! `docs/adr/` before any of them is implemented. No code exists yet.
