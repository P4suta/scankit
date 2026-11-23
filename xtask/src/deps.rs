// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `deps` gate for the deliberately tiny workspace graph.
//!
//! Product architecture no longer rides on Cargo packages: there is exactly one product package, `scan-kit`, plus the dependency-free repository tool `xtask`.
//! This gate makes that absence of package edges a maintained invariant.
//! Module-level direction inside the product is the `purity` gate's subject.

use std::fs;
use std::io;

use crate::shared::{self, Gate};

/// The `deps` gate, as the dispatcher sees it.
pub(crate) const GATE: Gate = Gate {
    name: "deps",
    purpose: "the workspace contains one product package and no inter-package edges",
    reference: "ARCHITECTURE.md",
    run,
};

/// The complete workspace package register.
const EXPECTED_MEMBERS: &[&str] = &["scan-kit", "xtask"];

/// Tools invoked as external programs and never linked into a package.
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

fn run() -> io::Result<Vec<String>> {
    let members = shared::members()?;
    let mut violations = Vec::new();

    for expected in EXPECTED_MEMBERS {
        if !members.iter().any(|member| member.name == *expected) {
            violations.push(format!(
                "workspace is missing frozen member `{expected}`; expected exactly {EXPECTED_MEMBERS:?}"
            ));
        }
    }
    for member in &members {
        if !EXPECTED_MEMBERS.contains(&member.name.as_str()) {
            violations.push(format!(
                "{} is an extra package; product boundaries are private modules, not crates",
                member.name,
            ));
        }
    }

    for member in &members {
        let manifest = fs::read_to_string(member.directory.join("Cargo.toml"))?;
        for dependency in shared::declared_dependencies(&manifest) {
            if NEVER_A_DEP.contains(&dependency.name.as_str()) {
                violations.push(format!(
                    "{} declares `{}`, a repository tool that must not be linked",
                    member.name, dependency.name,
                ));
            }
            if members
                .iter()
                .any(|candidate| candidate.name == dependency.name)
            {
                violations.push(format!(
                    "{} depends on workspace package `{}`; the workspace graph must stay edgeless",
                    member.name, dependency.name,
                ));
            }
        }
    }

    Ok(violations)
}

#[cfg(test)]
mod tests {
    use super::{EXPECTED_MEMBERS, run};

    #[test]
    fn the_register_has_one_product_and_one_tool() {
        assert_eq!(EXPECTED_MEMBERS, ["scan-kit", "xtask"]);
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
