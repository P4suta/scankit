// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `skan` — scan a page from the command line.
//!
//! A thin front end over the `scankit` facade: discover scanners, print capabilities,
//! and run a scan to files. The first exit criterion it must meet is M1's —
//! `skan scan --backend virtual` producing a multi-page fixture from an ADF script
//! (`ROADMAP.md`) — so the virtual backend is a selectable device here from the start,
//! not a hidden test rig.
//!
//! # Status
//!
//! Bootstrap. No subcommand is implemented yet; see `ROADMAP.md` (M1).

fn main() {}
