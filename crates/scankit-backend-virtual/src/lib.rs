// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A virtual scanner: a full implementation of the `scankit_core` traits against a
//! script instead of a device. The primary verification ground of the whole stack
//! (`docs/adr/0011`), and a first-class backend rather than a test fixture.
//!
//! A `Scenario` is built with a consuming builder and describes everything the virtual
//! device will do:
//!
//! - The capabilities it advertises, per input source.
//! - The pages an ADF run produces — several pages, each with its declared format and
//!   bytes, so multi-page delivery is exercised end to end.
//! - The failures it stages: a jam after page two, a cover opened mid-job, an empty
//!   feeder — each surfacing as the `Intervention` class the vocabulary defines
//!   (`docs/adr/0006`).
//! - A `hang()` hook: a point at which the device goes silent, so cancellation,
//!   drop-abandonment, and timeout behavior in the layers above can be tested
//!   deterministically.
//!
//! Because this crate depends only on `scankit-core`, everything the facade and the
//! CLI do — enumeration, job lifecycle, page delivery, error mapping — runs against it
//! with no network and no hardware, in ordinary `cargo test`.
//!
//! # Invariants
//!
//! - No I/O and no clock (`just purity`): a scripted hang is a state, not a sleep.
//! - Scenario behavior is deterministic: the same script produces the same transcript,
//!   every run.
//!
//! # Status
//!
//! Frozen at bootstrap: the shapes above are the day-one design, recorded in
//! `docs/adr/` before any of them is implemented. No code exists yet.
