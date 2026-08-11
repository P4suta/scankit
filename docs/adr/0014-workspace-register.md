# ADR-0014: the workspace register

- Status: accepted
- Date: 2026-08-11

## Context

This workspace is the sixth in a family (kumihan, wimkit, icalkit, roffkit, bufrkit)
that shares one register: the same layout, the same gate philosophy, the same release
posture. The register exists because every deviation between siblings is a thing a
maintainer must remember, and because the traps it encodes were each paid for once —
a bin name colliding with a lib name killing `cargo doc`, a `doc-valid-idents` list
silently replacing Clippy's defaults, release automation publishing something a merge
never meant to publish.

Recording the register as an ADR makes conformance checkable and deviation citable:
a sibling that departs from it says where and why, in its own ADR.

## Decision

The workspace is configured as follows, and these values are frozen:

- **Toolchain and shape.** Edition 2024, `rust-version = "1.85"` (the floor edition
  2024 imposes), `resolver = "3"`, workspace `version = "0.0.0"` until a deliberate
  first release. `fuzz/` is excluded from the workspace (cargo-fuzz needs nightly).
  `profile.release` uses `lto = "thin"` and `codegen-units = 1`.
- **Lints.** Clippy pedantic at `warn` with the panic family, `arithmetic_side_effects`
  and `indexing_slicing` warned (network-parser workspace), configured centrally in
  `Cargo.toml` and `clippy.toml`; `doc-valid-idents` keeps `".."` as its last entry.
  There is no `forbid(unsafe_code)`; instead `unsafe_op_in_unsafe_fn = "deny"` plus
  the `unsafe-boundary` gate with an empty allowlist, which is stricter and survives
  the first legitimate exception.
- **Release posture.** release-plz with everything off (`release_always = false`,
  explicit `publish = false`, no tags, no releases, no changelog automation) and all
  crates in one `version_group`; the facade alone carries the changelog. Publishing is
  a decision, never a side effect.
- **Gates.** `just ci` runs offline and predicts CI exactly. CI SHA-pins every action
  with the version in a comment, runs one `just` recipe per job, applies
  `RUSTFLAGS=-D warnings` per job (never to msrv), and ends in a single `ci-required`
  aggregation job that branch protection references. Tool versions live in `mise.toml`
  and must match CI's pins; hooks install via `mise run hooks` (lefthook, with typos
  running `--force-exclude`).
- **Licensing.** MIT OR Apache-2.0, REUSE-compliant: every source and config file
  opens with the three-line SPDX header; Markdown, `Cargo.lock`, `.gitignore`, and
  CODEOWNERS are covered by aggregate annotations in `REUSE.toml`.
- **Language.** Code, comments, documentation, and commit messages are English;
  `typos` runs `locale = "en-us"` with no spelling allowlist. Commits are Conventional
  Commits, validated by `committed`.

The gate is mechanical, not cultural. `just ci` is the register's own enforcement —
`fmt-check`, `toml-check`, `typos`, `lint`, `deps`, `purity`, `placeholder`,
`unsafe-boundary`, `bin-name`, `deny`, `shear`, `reuse`, `actionlint`, `zizmor`, and
`msrv` in one recipe — and the `ci-required` job makes the same set the merge bar.

## Consequences

A contributor from any sibling workspace is at home here: the commands, the layout,
and the failure messages are the same, and the family's accumulated trap knowledge
applies without translation.

The register's deliberate absences are as binding as its presences: no `no-std`/`wasm`
lanes (the cores are `std` by design, ADR 0003) and no cargo-mutants lane yet
(mutation testing starts when logic exists to mutate) — both recorded as comments in
the Justfile so their absence reads as a decision rather than an oversight.

Raising the MSRV, changing the edition, or turning on any release automation is an
edit to this ADR first.

Every gate in the register was written and green at bootstrap, before the code it
governs existed.
