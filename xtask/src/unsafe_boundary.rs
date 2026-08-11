// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `unsafe-boundary` gate.
//!
//! The workspace deliberately does not write `#![forbid(unsafe_code)]` into every
//! crate: a blanket forbid is the kind of rule that gets deleted wholesale the first
//! time a platform shim genuinely needs `unsafe`, and a deleted rule protects nothing.
//! This gate is the replacement, and it is stricter where it matters:
//!
//! 1. No source outside an allowlisted directory contains the token `unsafe` at all.
//!    The allowlist ([`UNSAFE_DIRECTORIES`]) is empty today; a future entry is an ADR,
//!    not an edit.
//! 2. Inside an allowlisted directory, every `unsafe {` block is immediately preceded
//!    by a comment block carrying a `// SAFETY:` line.
//! 3. No `unsafe fn` is declared anywhere, allowlisted or not: a boundary module
//!    exports nothing unsafe to call, so a caller cannot get one wrong. The workspace
//!    lint `unsafe_op_in_unsafe_fn = "deny"` is the compiler-side half of the same
//!    rule.
//!
//! Comments and string literals are excluded the same deliberately crude way the rest
//! of this tool reads Rust — a line whose first non-space characters are `//` is prose,
//! and a span between two `"` is data. That is enough, because the rule is about
//! *code*, and this file's own violation messages name the token they look for.

use std::fs;
use std::io;

use crate::shared::{self, Gate};

/// The `unsafe-boundary` gate, as the dispatcher sees it.
pub(crate) const GATE: Gate = Gate {
    name: "unsafe-boundary",
    purpose: concat!(
        "no product source says `unsafe` outside the allowlisted boundary, and the ",
        "boundary (empty today) documents every block"
    ),
    reference: "docs/adr/0014-workspace-register.md",
    run,
};

/// The directories in which `unsafe` may appear, relative to the repository root.
///
/// Empty on purpose: no crate in this workspace needs `unsafe` today. The first one
/// that does — a platform trust-store shim in `scankit-http` is the plausible
/// candidate — earns its entry together with an ADR recording why, and rules 2 and 3
/// start applying to it the moment the entry lands.
const UNSAFE_DIRECTORIES: &[&str] = &[];

/// The comment every `unsafe` block must be preceded by.
const SAFETY: &str = "// SAFETY:";

/// Check that `unsafe` lives only where the policy says, and behaves there.
fn run() -> io::Result<Vec<String>> {
    let root = shared::workspace_root()?;
    let allowed: Vec<_> = UNSAFE_DIRECTORIES
        .iter()
        .map(|directory| root.join(directory))
        .collect();
    let mut violations = Vec::new();
    let mut examined: usize = 0;
    for source in shared::rust_sources(&root.join("crates"))?
        .into_iter()
        .chain(shared::rust_sources(&root.join("xtask").join("src"))?)
    {
        examined = examined.saturating_add(1);
        let name = shared::relative_name(&source, &root).replace('\\', "/");
        let text = fs::read_to_string(&source)?;
        if allowed
            .iter()
            .any(|directory| source.starts_with(directory))
        {
            violations.extend(check_inside_boundary(&name, &text));
        } else {
            violations.extend(check_outside_boundary(&name, &text));
        }
    }
    println!(
        "{name}: examined {examined} source file(s)",
        name = GATE.name
    );
    Ok(violations)
}

/// Rule 1, applied to a file that may not say `unsafe` at all.
fn check_outside_boundary(name: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (at, line) in text.lines().enumerate() {
        let Some(code) = code_of(line) else {
            continue;
        };
        if shared::holds_token(&code, "unsafe") {
            violations.push(format!(
                "{name}:{line}: `unsafe` outside the allowlisted boundary (empty today; \
                 an entry requires an ADR)",
                line = at.saturating_add(1)
            ));
        }
    }
    violations
}

/// Rules 2 and 3, applied to a file inside the allowlisted boundary.
fn check_inside_boundary(name: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (at, line) in lines.iter().enumerate() {
        let Some(code) = code_of(line) else {
            continue;
        };
        // The message stays on the quoted opening line: this gate reads its own source,
        // and a string continuation line would present the bare token as code.
        if shared::holds_token(&code, "unsafe fn") {
            violations.push(format!(
                "{name}:{line}: declares an `unsafe fn`, which the boundary must not export",
                line = at.saturating_add(1)
            ));
        }
        if !code.contains("unsafe {") {
            continue;
        }
        if !documented_above(&lines, at) {
            violations.push(format!(
                "{name}:{line}: an `unsafe {{` whose comment block above it holds no \
                 `{SAFETY}` line",
                line = at.saturating_add(1)
            ));
        }
    }
    violations
}

/// One line with its comment and its string literals removed, or `None` for pure prose.
fn code_of(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('*') {
        return None;
    }
    let mut code = String::new();
    let mut inside_string = false;
    let mut previous = ' ';
    for character in trimmed.chars() {
        if character == '"' && previous != '\\' {
            inside_string = !inside_string;
            previous = character;
            continue;
        }
        if !inside_string {
            code.push(character);
        }
        previous = character;
    }
    Some(code)
}

/// Whether the contiguous comment block immediately above `at` carries a `// SAFETY:`
/// line.
///
/// The block rather than the single line above, because a comment that establishes
/// pointer provenance, initialization, aliasing, and the failure contract does not fit
/// on one line and should not be written as though it did.
fn documented_above(lines: &[&str], at: usize) -> bool {
    let mut above = at;
    while let Some(before) = above.checked_sub(1) {
        let Some(line) = lines.get(before) else {
            return false;
        };
        let trimmed = line.trim_start();
        if !trimmed.starts_with("//") {
            return false;
        }
        if trimmed.starts_with(SAFETY) {
            return true;
        }
        above = before;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{check_inside_boundary, check_outside_boundary, run};

    #[test]
    fn an_unsafe_token_outside_the_boundary_is_a_violation() {
        let text = "pub fn read() {\n    unsafe { *pointer }\n}\n";
        let violations = check_outside_boundary("crates/x/src/lib.rs", text);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations.first().is_some_and(|v| v.contains(":2:")));
    }

    #[test]
    fn prose_and_strings_naming_unsafe_are_not_violations() {
        let text = "// unsafe is forbidden here\nconst NOTE: &str = \"unsafe\";\n";
        assert!(check_outside_boundary("x.rs", text).is_empty());
    }

    #[test]
    fn an_undocumented_block_inside_the_boundary_is_a_violation() {
        let text = "fn read() {\n    unsafe { call() }\n}\n";
        let violations = check_inside_boundary("sys.rs", text);
        assert_eq!(violations.len(), 1, "{violations:?}");
    }

    #[test]
    fn an_unsafe_fn_is_a_violation_even_inside_the_boundary() {
        let text = "pub unsafe fn raw() {}\n";
        let violations = check_inside_boundary("sys.rs", text);
        assert_eq!(violations.len(), 1, "{violations:?}");
    }

    #[test]
    fn a_documented_block_inside_the_boundary_passes() {
        let text = concat!(
            "fn read() {\n",
            "    // SAFETY: the handle was checked non-null above and is not shared.\n",
            "    unsafe { call() }\n",
            "}\n"
        );
        assert!(check_inside_boundary("sys.rs", text).is_empty());
    }

    #[test]
    fn the_workspace_as_committed_passes_the_gate() {
        let violations = run().expect("the workspace sources are readable");
        assert!(
            violations.is_empty(),
            "run `cargo run -p xtask -- unsafe-boundary`: {violations:?}"
        );
    }
}
