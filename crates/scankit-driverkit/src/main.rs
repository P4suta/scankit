// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `scandev` — probe a device, capture its traffic, and turn quirks into fixtures.
//!
//! The workbench for interoperability work: talk to a real scanner at a low level,
//! record request/reply cassettes for `scankit-testkit` to replay, dump raw
//! capability documents, and compare observed behavior against what `sane-airscan`
//! documents for the same model (`docs/adr/0012`). It may reach past the facade into
//! the protocol and transport crates — the one consumer allowed to, because its job
//! is to see what the facade abstracts away.
//!
//! # Status
//!
//! Bootstrap. No subcommand is implemented yet; see `ROADMAP.md` (M3).

fn main() {}
