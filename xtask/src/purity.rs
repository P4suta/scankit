// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `purity` gate: crate-strength separation inside one product package.
//!
//! Cargo is intentionally not the architecture boundary.
//! The domain model and pure protocol directory are a lower layer, while drivers, discovery, HTTP, clocks, and sockets are above them.
//! This gate rejects an upward module edge or an I/O/clock token in the lower layer.
//! Rust privacy keeps the public surface closed; this check keeps private dependency direction closed.

use std::fs;
use std::io;
use std::path::Path;

use crate::shared::{self, Gate};

/// The `purity` gate, as the dispatcher sees it.
pub(crate) const GATE: Gate = Gate {
    name: "purity",
    purpose: "domain and protocol modules reach no transport, socket, or clock",
    reference: "ARCHITECTURE.md and docs/adr/0003-sans-io-protocol-cores.md",
    run,
};

/// Tokens forbidden from every lower-layer source.
const FORBIDDEN: &[&str] = &[
    "crate::api",
    "crate::driver",
    "crate::transport",
    "std::fs",
    "std::net",
    "std::process",
    "std::thread",
    "Instant",
    "SystemTime",
    "mdns_sd",
    "rustls",
    "ureq",
    "url::",
];

/// Additional tokens forbidden specifically from protocol grammar/state code.
const PROTOCOL_FORBIDDEN: &[&str] = &["std::io", "Read", "Write"];

fn run() -> io::Result<Vec<String>> {
    let products = shared::product_crates()?;
    let Some(product) = products.first() else {
        return Ok(vec!["workspace contains no product package".to_owned()]);
    };
    let mut violations = Vec::new();
    if products.len() != 1 || product.name != "scan-kit" {
        violations.push(format!(
            "purity expects exactly the `scan-kit` product package, found {:?}",
            products
                .iter()
                .map(|member| member.name.as_str())
                .collect::<Vec<_>>(),
        ));
        return Ok(violations);
    }

    let source = product.directory.join("src");
    let mut files = vec![
        source.join("error.rs"),
        source.join("model.rs"),
        source.join("wire.rs"),
    ];
    files.extend(shared::rust_sources(&source.join("protocol"))?);

    for file in &files {
        if !file.is_file() {
            violations.push(format!(
                "required lower-layer source {} is missing",
                relative(file, &product.directory),
            ));
            continue;
        }
        inspect(file, &product.directory, &mut violations)?;
    }

    let lib = fs::read_to_string(source.join("lib.rs"))?;
    for (line_number, line) in shared::code_only(&lib).lines().enumerate() {
        if line.trim_start().starts_with("pub mod ") {
            violations.push(format!(
                "src/lib.rs:{} exposes an implementation module; re-export domain names individually",
                line_number.saturating_add(1),
            ));
        }
    }

    println!(
        "{}: examined {} lower-layer source file(s)",
        GATE.name,
        files.len(),
    );
    Ok(violations)
}

fn inspect(file: &Path, root: &Path, violations: &mut Vec<String>) -> io::Result<()> {
    let source = shared::code_only(&fs::read_to_string(file)?);
    let is_protocol = file
        .components()
        .any(|component| component.as_os_str() == "protocol");
    for (line_number, line) in source.lines().enumerate() {
        for token in FORBIDDEN.iter().chain(
            is_protocol
                .then_some(PROTOCOL_FORBIDDEN)
                .into_iter()
                .flatten(),
        ) {
            if line.contains(token) && shared::holds_token(line, token) {
                violations.push(format!(
                    "{}:{} names `{token}`; lower layers cannot reach I/O or implementation policy",
                    relative(file, root),
                    line_number.saturating_add(1),
                ));
            }
        }
    }
    Ok(())
}

fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{FORBIDDEN, PROTOCOL_FORBIDDEN, run};

    #[test]
    fn private_upper_layers_are_explicitly_forbidden() {
        for edge in ["crate::api", "crate::driver", "crate::transport"] {
            assert!(FORBIDDEN.contains(&edge));
        }
        assert!(PROTOCOL_FORBIDDEN.contains(&"std::io"));
    }

    #[test]
    fn the_workspace_as_committed_passes_the_gate() {
        let violations = run().expect("the workspace sources are readable");
        assert!(
            violations.is_empty(),
            "run `cargo run -p xtask -- purity`: {violations:?}"
        );
    }
}
