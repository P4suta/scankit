# SPDX-FileCopyrightText: 2026 scankit contributors
#
# SPDX-License-Identifier: MIT OR Apache-2.0

# `just --list` shows only the comment line directly above a recipe, so rationale goes in
# a block above a blank line and the line touching the recipe is its one-line summary.

# NOTE: there is no `no-std` and no `wasm` recipe here on purpose: the pure crates are
# `std` (I/O-free by the `purity` and `deps` gates, not by libc absence), so a bare-metal
# or wasm lane would prove nothing this design claims. There is also no cargo-mutants
# lane yet: mutation testing earns its cost once logic exists to mutate.

set shell := ["sh", "-cu"]
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

export RUSTDOCFLAGS := "-D warnings"

# List the available development commands.
default:
    @just --list

# Format the workspace.
fmt:
    cargo fmt --all

# Check formatting without modifying files.
fmt-check:
    cargo fmt --all --check

# Check TOML formatting.
toml-check:
    taplo fmt --check --diff

# Run Clippy with and without default features across every target.
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo clippy --workspace --all-targets --no-default-features -- -D warnings

# Nextest runs normal tests process-per-test; Cargo separately runs doctests, which
# nextest does not currently support.

# Run the workspace test suite.
test:
    cargo nextest run --workspace --all-features
    cargo test --workspace --doc --all-features

# Run the workspace test suite with the non-fail-fast CI profile.
test-ci:
    cargo nextest run --profile ci --workspace --all-features
    cargo test --workspace --doc --all-features

# Build public documentation with warnings denied.
doc:
    cargo doc --workspace --all-features --no-deps

# No crate declares a feature yet, so this is one pass today. The powerset lane over the
# protocol cores is added together with their first feature.

# Compile every individual feature combination cargo-hack derives.
feature-matrix:
    cargo hack check --workspace --all-targets --each-feature

# Reject any inter-crate edge the frozen ALLOWED matrix does not carry (ARCHITECTURE.md).
deps:
    cargo run --quiet -p xtask -- deps

# Reject sockets, clocks, and third parties in the sans-I/O crates (docs/adr/0003).
purity:
    cargo run --quiet -p xtask -- purity

# Reject todo!/unimplemented! bodies and allow/expect suppressions (CONTRIBUTING.md).
placeholder:
    cargo run --quiet -p xtask -- placeholder

# Reject `unsafe` outside the allowlisted boundary, which is empty (docs/adr/0014).
unsafe-boundary:
    cargo run --quiet -p xtask -- unsafe-boundary

# Reject a bin target that shares a name with any lib crate (docs/adr/0015).
bin-name:
    cargo run --quiet -p xtask -- bin-name

# Spell-check the repository.
typos:
    typos

# Check dependency advisories, bans, licenses, and sources.
deny:
    cargo deny --all-features check advisories bans licenses sources

# Reject unused, misplaced, and unlinked Cargo dependencies or source files.
shear:
    cargo shear --deny-warnings

# Check REUSE/SPDX compliance.
reuse:
    uvx --with charset-normalizer==3.4.9 reuse==6.2.0 lint

# Validate GitHub Actions workflows.
actionlint:
    actionlint -color

# The auditor gets neither network nor repository credentials.

# Reject high-severity GitHub Actions and Dependabot security findings.
zizmor:
    zizmor --offline --persona regular --min-severity high .

# Verify every workspace crate at the shared declared MSRV.
msrv:
    cargo msrv verify --path crates/scankit-core
    cargo msrv verify --path crates/scankit-testkit
    cargo msrv verify --path crates/scankit-escl
    cargo msrv verify --path crates/scankit-wsd
    cargo msrv verify --path crates/scankit-http
    cargo msrv verify --path crates/scankit-discover
    cargo msrv verify --path crates/scankit-backend-escl
    cargo msrv verify --path crates/scankit-backend-wsd
    cargo msrv verify --path crates/scankit-backend-virtual
    cargo msrv verify --path crates/scankit
    cargo msrv verify --path crates/scankit-cli
    cargo msrv verify --path crates/scankit-driverkit
    cargo msrv verify --path xtask

# Fast deterministic checks used during the edit/commit loop.
check: fmt-check toml-check typos lint deps purity placeholder unsafe-boundary bin-name shear reuse actionlint zizmor
    @echo "fast local checks passed"

# Every gate below runs offline, so a green `just ci` predicts a green pipeline.

# Every CI gate, exactly as the pipeline runs it.
ci: fmt-check toml-check typos lint feature-matrix test-ci doc deps purity placeholder unsafe-boundary bin-name deny shear reuse actionlint zizmor msrv
    @echo "local CI passed"
