// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `deps` gate: the one rule, and the crate-level invariants around it.
//!
//! ARCHITECTURE.md's premise is that dependencies flow only toward the leaves. `cargo
//! fmt` and clippy do not check that; this does. The check reads manifest text through
//! the shared readers, because this tool declares no dependencies and therefore has no
//! TOML parser; a `package = "..."` rename — the one spelling that could smuggle a
//! workspace edge past a name check — is resolved by the reader itself.
//!
//! The rules, keyed to the crate they defend:
//!
//! * **R1 — the one rule.** A workspace member's normal or build dependency on another
//!   member must be an arrow [`ALLOWED`] carries.
//! * **R2 — the vocabulary's dependency-freedom.** A [`ZERO_DEP`] crate declares no
//!   normal or build dependency of any kind, workspace or not (`docs/adr/0003`).
//! * **R3 — no dev cycle.** A dev-dependency ships in nothing, so the one rule does not
//!   reach it; it may not close a cycle, which would state the architecture backwards.
//! * **R5 — the testkit is dev-only.** It is `publish = false` and must reach no
//!   shipped artifact.
//! * **R7 — total coverage.** Every workspace member has an [`ALLOWED`] row, and every
//!   row names a member, so neither the workspace nor this table can grow past the
//!   other unnoticed.
//!
//! There is no R4 and no R6 here by design: the unsafe policy is the `unsafe-boundary`
//! gate's whole subject, and third-party purity is the `purity` gate's.

use std::fs;
use std::io;

use crate::shared::{self, DepKind, Gate};

/// The `deps` gate, as the dispatcher sees it.
pub(crate) const GATE: Gate = Gate {
    name: "deps",
    purpose: concat!(
        "every inter-crate dependency is an arrow the frozen graph carries, the ",
        "vocabulary depends on nothing, and the testkit ships in nothing"
    ),
    reference: "ARCHITECTURE.md and docs/adr/0003-sans-io-protocol-cores.md",
    run,
};

/// Which workspace crates each crate may name. **Transitively closed**, so a single
/// lookup answers "may `from` name `to`, directly or through anything it names" (see
/// [`reaches`]), and the closure is asserted by this module's tests.
///
/// The rows are the intended graph, transcribed rather than derived: deriving it from
/// the manifests would make the gate agree with whatever the manifests happen to say.
/// The intended *direct* edges are narrower and live in ARCHITECTURE.md — for example
/// `scankit-cli` names only `scankit` directly; its row here is wide only because
/// closure requires everything the facade may name.
const ALLOWED: &[(&str, &[&str])] = &[
    ("scankit-core", &[]),
    ("scankit-testkit", &["scankit-core"]),
    ("scankit-escl", &["scankit-core"]),
    ("scankit-wsd", &["scankit-core"]),
    ("scankit-http", &["scankit-core"]),
    ("scankit-discover", &["scankit-core", "scankit-wsd"]),
    (
        "scankit-backend-escl",
        &["scankit-core", "scankit-escl", "scankit-http"],
    ),
    (
        "scankit-backend-wsd",
        &["scankit-core", "scankit-wsd", "scankit-http"],
    ),
    ("scankit-backend-virtual", &["scankit-core"]),
    (
        "scankit",
        &[
            "scankit-core",
            "scankit-escl",
            "scankit-wsd",
            "scankit-http",
            "scankit-discover",
            "scankit-backend-escl",
            "scankit-backend-wsd",
            "scankit-backend-virtual",
        ],
    ),
    (
        "scankit-cli",
        &[
            "scankit",
            "scankit-core",
            "scankit-escl",
            "scankit-wsd",
            "scankit-http",
            "scankit-discover",
            "scankit-backend-escl",
            "scankit-backend-wsd",
            "scankit-backend-virtual",
        ],
    ),
    (
        "scankit-driverkit",
        &[
            "scankit",
            "scankit-core",
            "scankit-escl",
            "scankit-wsd",
            "scankit-http",
            "scankit-discover",
            "scankit-backend-escl",
            "scankit-backend-wsd",
            "scankit-backend-virtual",
        ],
    ),
    ("xtask", &[]),
];

/// The crates whose dependency-freedom is a fixed architectural rule.
///
/// `scankit-core` is the shared vocabulary: every crate in the stack speaks its types,
/// so anything it depended on would be a dependency of everything (`docs/adr/0003`).
/// Note the consequence the tests below spell out: `scankit-testkit` may name the
/// vocabulary, so the vocabulary may not even *dev*-depend on the testkit — its own
/// tests use `std` alone, and trait-level behavior is tested from the crates above.
const ZERO_DEP: &[&str] = &["scankit-core"];

/// Crates that may be named only from a dev-dependency table.
const DEV_ONLY: &[&str] = &["scankit-testkit"];

/// Tools the workspace invokes as external programs and never links. They read the tree
/// or the lockfile; naming one as a dependency is an error.
const NEVER_A_DEP: &[&str] = &[
    "cargo-nextest",
    "cargo-hack",
    "cargo-deny",
    "cargo-shear",
    "cargo-msrv",
    "release-plz",
    "committed",
    "just",
    "taplo-cli",
    "typos-cli",
];

/// Whether `from` may name `to`, directly or through anything it may name.
///
/// One lookup, because [`ALLOWED`] is transitively closed.
fn reaches(from: &str, to: &str) -> bool {
    ALLOWED
        .iter()
        .find(|(name, _)| *name == from)
        .is_some_and(|(_, allowed)| allowed.contains(&to))
}

/// Check every workspace member against the rules above.
fn run() -> io::Result<Vec<String>> {
    let members = shared::members()?;
    let mut violations = Vec::new();

    // R7, both directions.
    for member in &members {
        if !ALLOWED.iter().any(|(name, _)| *name == member.name) {
            violations.push(format!(
                "{name}: workspace member has no row in xtask/src/deps.rs ALLOWED; add \
                 one so the one rule reaches it",
                name = member.name
            ));
        }
    }
    for (krate, _) in ALLOWED {
        if !members.iter().any(|member| member.name == *krate) {
            violations.push(format!(
                "{krate}: has an ALLOWED row but is not a workspace member; the table in \
                 xtask/src/deps.rs has gone stale"
            ));
        }
    }

    for member in &members {
        let Some((_, allowed)) = ALLOWED.iter().find(|(name, _)| *name == member.name) else {
            continue;
        };
        let manifest = fs::read_to_string(member.directory.join("Cargo.toml"))?;
        for declared in shared::declared_dependencies(&manifest) {
            let dep = declared.name.as_str();
            let krate = member.name.as_str();

            // No external tool is a dependency: the workspace invokes them, it does not
            // link them.
            if NEVER_A_DEP.contains(&dep) {
                violations.push(format!(
                    "{krate}: declares `{dep}`, a tool the workspace invokes; it belongs \
                     in mise.toml or a CI step, not a dependency table"
                ));
                continue;
            }

            // R2 — the vocabulary declares nothing that ships, workspace or not.
            if ZERO_DEP.contains(&krate) && declared.kind != DepKind::Development {
                violations.push(format!(
                    "{krate}: declares `{dep}`; this crate's dependency-freedom is \
                     architecture (docs/adr/0003) — put the dependency in a crate that \
                     may hold it"
                ));
                continue;
            }

            let is_member = members.iter().any(|other| other.name == dep);
            if !is_member {
                // Third-party names are the purity gate's subject, not this one's.
                continue;
            }

            // R5 — the testkit is a dev-dependency and nothing else.
            if DEV_ONLY.contains(&dep) && declared.kind != DepKind::Development {
                violations.push(format!(
                    "{krate}: names `{dep}` outside a dev-dependency table; it is \
                     `publish = false` and must reach no shipped artifact"
                ));
                continue;
            }

            match declared.kind {
                // R1 — the one rule. A shipped dependency must be an arrow the graph has.
                DepKind::Normal | DepKind::Build if !allowed.contains(&dep) => {
                    violations.push(format!(
                        "{krate}: depends on `{dep}`, an arrow the frozen graph does not \
                         carry; lift the coupling to the facade (ARCHITECTURE.md, the \
                         one rule)"
                    ));
                },
                // R3 — a dev-dependency may not close a cycle: a crate whose tests are
                // written in the terms of something above it has inverted, whatever the
                // tarball holds.
                DepKind::Development if reaches(dep, &member.name) => {
                    violations.push(format!(
                        "{krate}: dev-depends on `{dep}`, which sits above it; the test \
                         states the architecture backwards"
                    ));
                },
                _ => {},
            }
        }
    }
    Ok(violations)
}

#[cfg(test)]
mod tests {
    use super::{ALLOWED, DEV_ONLY, ZERO_DEP, reaches, run};

    #[test]
    fn the_matrix_is_transitively_closed() {
        // The claim `reaches` rests on: if A may name B, then A may name everything B
        // may.
        for (krate, allowed) in ALLOWED {
            for dep in *allowed {
                let (_, deps_of_dep) = ALLOWED
                    .iter()
                    .find(|(name, _)| name == dep)
                    .unwrap_or_else(|| panic!("{krate} may name {dep}, which has no row"));
                for transitive in *deps_of_dep {
                    assert!(
                        allowed.contains(transitive),
                        "{krate} may name {dep}, which may name {transitive} — but \
                         {krate} may not. ALLOWED must be transitively closed."
                    );
                }
            }
        }
    }

    #[test]
    fn no_crate_may_name_itself() {
        for (krate, allowed) in ALLOWED {
            assert!(!allowed.contains(krate), "{krate} names itself");
        }
    }

    #[test]
    fn every_zero_dep_crate_has_an_empty_row() {
        // The two rules must agree: a crate that may depend on nothing has nothing in
        // its row.
        for krate in ZERO_DEP {
            let (_, allowed) = ALLOWED
                .iter()
                .find(|(name, _)| name == krate)
                .unwrap_or_else(|| panic!("{krate} is zero-dep but has no ALLOWED row"));
            assert!(
                allowed.is_empty(),
                "{krate} is zero-dep but may name {allowed:?}"
            );
        }
    }

    #[test]
    fn the_load_bearing_absences_hold() {
        // The edges whose absence the design leans on. Each would satisfy a
        // membership-only check; each is refused by the rows above.
        assert!(
            !reaches("scankit-escl", "scankit-http"),
            "the protocol core must not see the transport (docs/adr/0003)"
        );
        assert!(
            !reaches("scankit-wsd", "scankit-http"),
            "the protocol core must not see the transport (docs/adr/0003)"
        );
        assert!(
            !reaches("scankit-escl", "scankit-wsd"),
            "the two protocols share vocabulary, never each other"
        );
        assert!(
            !reaches("scankit-backend-virtual", "scankit-http"),
            "the virtual device needs no network (docs/adr/0011)"
        );
        assert!(
            !reaches("scankit-discover", "scankit-escl"),
            "discovery needs the WS-Discovery codec, not the scan protocols"
        );
    }

    #[test]
    fn a_dev_dependency_is_a_cycle_only_when_the_target_reaches_back() {
        // The arrows the tests are expected to use, and why each is fine.
        assert!(
            !reaches("scankit-testkit", "scankit-escl"),
            "escl -> testkit (dev) is not a cycle"
        );
        assert!(
            !reaches("scankit-testkit", "scankit-backend-escl"),
            "backend-escl -> testkit (dev) is not a cycle"
        );

        // And the arrow the rule exists to refuse: the vocabulary tested in a
        // consumer's terms. This is why `scankit-core` cannot dev-depend on the
        // testkit, stated here rather than discovered in a failing build.
        assert!(
            reaches("scankit-testkit", "scankit-core"),
            "core -> testkit (dev) must read as a cycle"
        );
        assert!(
            reaches("scankit", "scankit-core"),
            "core -> scankit (dev) must read as a cycle"
        );
    }

    #[test]
    fn dev_only_crates_have_rows_so_the_cycle_rule_can_see_them() {
        for krate in DEV_ONLY {
            assert!(
                ALLOWED.iter().any(|(name, _)| name == krate),
                "{krate} is dev-only but has no ALLOWED row"
            );
        }
    }

    #[test]
    fn the_workspace_as_committed_passes_the_gate() {
        let violations = run().expect("the workspace manifests are readable");
        assert!(
            violations.is_empty(),
            "run `cargo run -p xtask -- deps`: {violations:?}"
        );
    }
}
