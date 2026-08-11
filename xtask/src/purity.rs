// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `purity` gate.
//!
//! The sans-I/O claim of `docs/adr/0003`, made mechanical. The crates in
//! [`PURE_CRATES`] are the ones whose whole value is that they do nothing: the
//! vocabulary, the two protocol cores, and the virtual device. For each of them this
//! gate rejects:
//!
//! - a third-party dependency in any table, outside the crate's own short allowlist
//!   ([`ALLOWED_THIRD_PARTY`] — `quick-xml` for the protocol cores, nothing else
//!   anywhere), and
//! - a source token that reaches I/O or a clock: `std::net`, `std::fs`,
//!   `std::process`, `std::thread`, `Instant`, `SystemTime`.
//!
//! What is deliberately *not* rejected: `std` itself and `std::time::Duration`. These
//! crates are `std` — the design forgoes `no_std` in exchange for ordinary strings and
//! collections — and a `Duration` is a value a state machine may carry as data. What a
//! pure crate may never do is *observe* time or a socket; producing "now" is I/O, and
//! naming a deadline is not. That line is exactly the difference between a state
//! machine that can be tested from a transcript and one that cannot
//! (`docs/adr/0011`).
//!
//! Comments are stripped before the token scan, so prose naming a forbidden token is
//! not a violation; a string literal holding one would be hidden rather than invented,
//! which is the safe direction for this gate.

use std::fs;
use std::io;

use crate::shared::{self, Gate};

/// The `purity` gate, as the dispatcher sees it.
pub(crate) const GATE: Gate = Gate {
    name: "purity",
    purpose: concat!(
        "the vocabulary, the protocol cores, and the virtual device reach no socket, ",
        "no clock, and no third-party crate beyond their stated allowance"
    ),
    reference: "docs/adr/0003-sans-io-protocol-cores.md",
    run,
};

/// The crates held pure, with the third-party crates each may ever declare.
///
/// The allowance is written here rather than granted case by case, so widening it is a
/// visible edit to a shared file. `quick-xml` is the one dependency the protocol cores
/// are designed to take (`docs/adr/0003`); its two-crate resolved footprint is a
/// measured fact (`docs/M0-ground-truth.md`).
const ALLOWED_THIRD_PARTY: &[(&str, &[&str])] = &[
    ("scankit-core", &[]),
    ("scankit-escl", &["quick-xml"]),
    ("scankit-wsd", &["quick-xml"]),
    ("scankit-backend-virtual", &[]),
];

/// The names of the pure crates, for messages and membership checks.
const PURE_CRATES: &[&str] = &[
    "scankit-core",
    "scankit-escl",
    "scankit-wsd",
    "scankit-backend-virtual",
];

/// The source tokens that reach I/O or a clock, each matched as a whole path segment
/// or identifier.
///
/// `std::thread` covers both `spawn` and `sleep`; `Instant` and `SystemTime` cover the
/// clock reads whatever path they are imported through.
const FORBIDDEN_TOKENS: &[&str] = &[
    "std::net",
    "std::fs",
    "std::process",
    "std::thread",
    "Instant",
    "SystemTime",
];

/// Check manifests and sources for every pure crate.
fn run() -> io::Result<Vec<String>> {
    let members = shared::members()?;
    let mut violations = Vec::new();
    let mut examined: usize = 0;

    for pure in PURE_CRATES {
        let Some(member) = members.iter().find(|member| member.name == *pure) else {
            violations.push(format!(
                "{pure}: is on the purity roster but is not a workspace member; the \
                 roster in xtask/src/purity.rs has gone stale"
            ));
            continue;
        };
        let allowed = ALLOWED_THIRD_PARTY
            .iter()
            .find(|(name, _)| name == pure)
            .map(|(_, list)| *list)
            .unwrap_or_default();

        let manifest = fs::read_to_string(member.directory.join("Cargo.toml"))?;
        for declared in shared::declared_dependencies(&manifest) {
            let dep = declared.name.as_str();
            let is_member = members.iter().any(|other| other.name == dep);
            if is_member || allowed.contains(&dep) {
                continue;
            }
            violations.push(format!(
                "{pure}: declares third-party `{dep}`; a pure crate may declare only \
                 workspace crates{allowance} (docs/adr/0003)",
                allowance = if allowed.is_empty() {
                    String::new()
                } else {
                    format!(" and {allowed:?}")
                }
            ));
        }

        for source in shared::rust_sources(&member.directory.join("src"))? {
            examined = examined.saturating_add(1);
            let name = shared::relative_name(&source, &member.directory);
            let code = shared::code_only(&fs::read_to_string(&source)?);
            for (at, line) in code.lines().enumerate() {
                for token in FORBIDDEN_TOKENS {
                    if line.contains(token) && shared::holds_token(line, token) {
                        violations.push(format!(
                            "{pure}: {name}:{line_number} names `{token}`; a pure crate \
                             observes no socket and no clock (docs/adr/0003)",
                            line_number = at.saturating_add(1)
                        ));
                    }
                }
            }
        }
    }

    println!(
        "{name}: examined {examined} source file(s) in {crates} pure crate(s)",
        name = GATE.name,
        crates = PURE_CRATES.len()
    );
    Ok(violations)
}

#[cfg(test)]
mod tests {
    use super::{ALLOWED_THIRD_PARTY, PURE_CRATES, run};

    #[test]
    fn the_roster_and_the_allowance_table_agree() {
        for (name, _) in ALLOWED_THIRD_PARTY {
            assert!(PURE_CRATES.contains(name), "{name} has an allowance row");
        }
        for name in PURE_CRATES {
            assert!(
                ALLOWED_THIRD_PARTY.iter().any(|(each, _)| each == name),
                "{name} is pure but has no allowance row"
            );
        }
    }

    #[test]
    fn only_the_protocol_cores_are_allowed_anything() {
        for (name, allowed) in ALLOWED_THIRD_PARTY {
            match *name {
                "scankit-escl" | "scankit-wsd" => {
                    assert_eq!(*allowed, ["quick-xml"], "{name}: quick-xml alone");
                },
                _ => assert!(allowed.is_empty(), "{name} may declare no third party"),
            }
        }
    }

    #[test]
    fn the_workspace_as_committed_passes_the_gate() {
        let violations = run().expect("the workspace manifests and sources are readable");
        assert!(
            violations.is_empty(),
            "run `cargo run -p xtask -- purity`: {violations:?}"
        );
    }
}
