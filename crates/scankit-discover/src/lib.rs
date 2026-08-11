// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The discovery leaf. Finds scanners on the local network and reports them as typed
//! device records; the second of the two crates allowed to touch a socket.
//!
//! Two discovery families cover the driverless world, and they come in pairs with their
//! protocol crates — sane-airscan's own numbers show the pairing (zeroconf 1,032 SLOC
//! with mdns 972 beside it; wsd 878 with wsdd 1,377 beside it; `docs/M0-ground-truth.md`):
//!
//! - mDNS/DNS-SD for eSCL: browse `_uscan._tcp` and `_uscans._tcp`, planned over
//!   `mdns-sd`. The service records carry the resource root and TLS hint the eSCL
//!   backend needs.
//! - WS-Discovery for WSD: multicast `Probe` over UDP and collect `ProbeMatch`. The
//!   datagram bytes are encoded and decoded by `scankit-wsd`; this crate owns only the
//!   socket, so the codec stays testable without a network (`docs/adr/0003`).
//!
//! # Invariants
//!
//! - Discovery is enumeration, not selection: this crate reports what answered and
//!   never picks a device for the caller.
//! - The WS-Discovery message grammar lives in `scankit-wsd`; if a parsing decision
//!   appears in this crate, it is in the wrong crate.
//!
//! # Status
//!
//! Frozen at bootstrap: the shapes above are the day-one design, recorded in
//! `docs/adr/` before any of them is implemented. No code exists yet.
