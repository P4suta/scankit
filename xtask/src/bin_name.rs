// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `bin-name` gate.
//!
//! Every binary target name must differ from every library crate name in the
//! workspace. The failure it prevents is concrete: `cargo doc` writes both a library's
//! and a binary's documentation to `target/doc/<name>`, so a bin that shares a lib's
//! name makes parallel rustdoc invocations fight over one directory and the build
//! dies — found the hard way across this workspace family, which is why the facade is
//! `scankit` while the command is `skan` (`docs/adr/0015`).
//!
//! Two checks:
//!
//! 1. **No collision.** Every bin target name — explicit `[[bin]]` or implicit
//!    `src/main.rs` — is compared against every package name that has a library
//!    target.
//! 2. **The frozen names.** `docs/adr/0015` names the two commands; [`EXPECTED`]
//!    transcribes it, so renaming a binary is an ADR edit, not a drive-by.

use std::fs;
use std::io;

use crate::shared::{self, Gate};

/// The `bin-name` gate, as the dispatcher sees it.
pub(crate) const GATE: Gate = Gate {
    name: "bin-name",
    purpose: "no binary target shares a name with any library crate, and the frozen \
              command names hold",
    reference: "docs/adr/0015-naming.md",
    run,
};

/// The bin names `docs/adr/0015` freezes, per owning crate.
const EXPECTED: &[(&str, &str)] = &[("scankit-cli", "skan"), ("scankit-driverkit", "scandev")];

/// One binary target, attributed to the member that declares it.
#[derive(Debug)]
struct BinTarget {
    /// The package that owns the target.
    owner: String,
    /// The target name `cargo` will use.
    name: String,
}

/// Check every member's targets.
fn run() -> io::Result<Vec<String>> {
    let members = shared::members()?;
    let mut violations = Vec::new();

    let mut lib_names = Vec::new();
    let mut bins = Vec::new();
    for member in &members {
        let manifest = fs::read_to_string(member.directory.join("Cargo.toml"))?;
        let explicit = explicit_bin_names(&manifest);
        let has_main = member.directory.join("src").join("main.rs").is_file();
        if member.directory.join("src").join("lib.rs").is_file() {
            lib_names.push(member.name.clone());
        }
        if explicit.is_empty() && has_main {
            // Cargo's implicit bin target takes the package name.
            bins.push(BinTarget {
                owner: member.name.clone(),
                name: member.name.clone(),
            });
        }
        for name in explicit {
            bins.push(BinTarget {
                owner: member.name.clone(),
                name,
            });
        }
    }

    for bin in &bins {
        if lib_names.contains(&bin.name) {
            violations.push(format!(
                "{owner}: bin target `{name}` collides with the library crate of the \
                 same name; rustdoc writes both to target/doc/{name} (docs/adr/0015)",
                owner = bin.owner,
                name = bin.name
            ));
        }
    }

    for (owner, expected) in EXPECTED {
        let found: Vec<&str> = bins
            .iter()
            .filter(|bin| bin.owner == *owner)
            .map(|bin| bin.name.as_str())
            .collect();
        if found != [*expected] {
            violations.push(format!(
                "{owner}: expected exactly one bin target named `{expected}` \
                 (docs/adr/0015), found {found:?}"
            ));
        }
    }

    Ok(violations)
}

/// The names declared by `[[bin]]` tables, in manifest order.
///
/// Reads the one style this repository writes: a `[[bin]]` header followed by a
/// `name = "..."` line before the next table.
fn explicit_bin_names(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside_bin = false;
    for line in manifest.lines() {
        let line = shared::before_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            inside_bin = line == "[[bin]]";
            continue;
        }
        if inside_bin {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "name" {
                    if let Some(name) = shared::quoted_values(value).first() {
                        names.push((*name).to_owned());
                    }
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::{EXPECTED, explicit_bin_names, run};

    #[test]
    fn reads_bin_names_and_only_bin_names() {
        let manifest = concat!(
            "[package]\n",
            "name = \"scankit-cli\"\n",
            "[[bin]]\n",
            "name = \"skan\"\n",
            "path = \"src/main.rs\"\n",
            "[lints]\n",
            "name = \"decoy\"\n",
        );
        assert_eq!(explicit_bin_names(manifest), ["skan"]);
        assert!(explicit_bin_names("[package]\nname = \"x\"\n").is_empty());
    }

    #[test]
    fn the_frozen_names_differ_from_every_family_crate_name() {
        for (owner, name) in EXPECTED {
            assert!(
                !name.starts_with("scankit"),
                "{owner}: `{name}` must not share the crate family prefix, or a future \
                 crate could collide with it"
            );
        }
    }

    #[test]
    fn the_workspace_as_committed_passes_the_gate() {
        let violations = run().expect("the workspace manifests are readable");
        assert!(
            violations.is_empty(),
            "run `cargo run -p xtask -- bin-name`: {violations:?}"
        );
    }
}
