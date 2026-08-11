// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Repository maintenance tasks for the scankit workspace.
//!
//! Run with `cargo run -p xtask -- <task>`, or through the `Justfile` recipes.
//!
//! This file is the dispatcher and nothing else. Each task is one module exposing one
//! `GATE`: the name it is invoked by, the sentence its holding justifies, the document
//! to read when it does not hold, and the check itself. The table below is the whole
//! routing decision, so a task is added by writing a module and one line here.
//!
//! This binary has no dependencies: the tool that enforces "the vocabulary crate has no
//! dependencies" should not itself have any, so it reads manifests and sources as text
//! through the hand-rolled readers in `shared`.
//!
//! Every gate collects **all** violations and prints them together, so one run says
//! everything that is wrong. A gate that cannot run at all is reported on a separate
//! channel from a gate that ran and found nothing: silence must never be ambiguous.

mod bin_name;
mod deps;
mod placeholder;
mod purity;
mod shared;
mod unsafe_boundary;

use std::process::ExitCode;

use crate::shared::Gate;

/// Every task, in the order a reader meets them in ARCHITECTURE.md.
const GATES: &[Gate] = &[
    deps::GATE,
    purity::GATE,
    placeholder::GATE,
    unsafe_boundary::GATE,
    bin_name::GATE,
];

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let Some(task) = arguments.next() else {
        print_usage();
        return ExitCode::FAILURE;
    };
    let Some(gate) = GATES.iter().find(|gate| gate.name == task) else {
        eprintln!("xtask: unknown task `{task}`");
        print_usage();
        return ExitCode::FAILURE;
    };
    gate.report()
}

/// Print the available tasks and what each one states.
fn print_usage() {
    eprintln!("usage: cargo run -p xtask -- <task>");
    for gate in GATES {
        eprintln!(
            "  {name:<16}  {purpose}",
            name = gate.name,
            purpose = gate.purpose
        );
    }
}
