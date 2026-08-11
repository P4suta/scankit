// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The HTTP transport leaf. The one crate in the stack that opens a TCP connection and
//! speaks TLS, so that no other crate has to know how.
//!
//! The protocol cores emit `HttpCall` values and consume `HttpReply` values
//! (`scankit_core::exchange`); this crate is what turns one into the other against a
//! real scanner. The day-one implementation plan is `ureq` v3 with `rustls`, bridged
//! from its synchronous API onto the stack's native `async fn` surface by a worker
//! thread — measured at 26 unique crates against a hyper stack that starts at 15 and
//! then requires tokio (`docs/adr/0008`). Because every caller goes through the
//! `exchange` value types, replacing the client is not a one-way door.
//!
//! Scanners terminate TLS with self-signed certificates as a matter of course, so this
//! crate owns `TrustPolicy`:
//!
//! - `SystemRoots` — the platform trust store, for the rare scanner with a real chain.
//! - `TrustOnFirstUse` — pin whatever the device presents on first contact and refuse
//!   any later change.
//! - `Fingerprint` — trust exactly one caller-supplied certificate fingerprint.
//!
//! There is no accept-all variant and no permissive default; a caller states its policy
//! or does not connect over TLS (`docs/adr/0009`).
//!
//! # Invariants
//!
//! - Only this crate (and `scankit-discover`, for datagrams) touches `std::net` or a
//!   TLS implementation; the deps rows in `xtask/src/deps.rs` keep every HTTP client
//!   crate out of everything else.
//! - Time spent waiting belongs to the pumps; this crate exposes deadlines as
//!   parameters, it does not invent them.
//!
//! # Status
//!
//! Frozen at bootstrap: the shapes above are the day-one design, recorded in
//! `docs/adr/` before any of them is implemented. No code exists yet.
