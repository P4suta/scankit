// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Scan a page from a network scanner in pure Rust. This is the crate to depend on.
//!
//! scankit is a driverless-first scanner stack: eSCL and WSD, discovered over
//! mDNS/DNS-SD and WS-Discovery, spoken over HTTP with an explicit TLS trust policy.
//! The product is the Rust API itself — this is not a SANE clone and exposes no C ABI
//! (`docs/adr/0001`, `docs/adr/0002`). This facade will hold, and nothing else will
//! hold:
//!
//! - The re-exported vocabulary of `scankit-core`, so one dependency line is enough.
//! - The `Composite` backend: eSCL, WSD, and the virtual device behind one uniform
//!   `Backend` surface, with eSCL preferred where a device offers both.
//! - The one-call API: discover, pick, validate the ticket, scan, deliver pages — the
//!   ten lines every application would otherwise write, written once.
//!
//! Pages are delivered as the device produced them: bytes plus a declared format,
//! never decoded, never re-encoded (`docs/adr/0007`).
//!
//! # Invariants
//!
//! - This crate composes; it implements no protocol and owns no socket. Grammar
//!   belongs to the protocol crates, sockets to the leaves, time to the pumps.
//! - Release-wise this crate is the family's changelog carrier: the version and
//!   CHANGELOG.md move here, for every crate at once (release-plz.toml).
//!
//! # Status
//!
//! Frozen at bootstrap: the shapes above are the day-one design, recorded in
//! `docs/adr/` before any of them is implemented. No code exists yet. Note that the
//! crates.io name `scankit` is occupied by an unrelated project
//! (`docs/M0-ground-truth.md`); the publication name is decided at first release,
//! the API and this workspace name are not hostage to it (`docs/adr/0015`).
