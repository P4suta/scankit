# ADR-0015: naming

- Status: accepted
- Date: 2026-08-11

## Context

Names in a Cargo workspace are load-bearing in one non-obvious way: `cargo doc` writes
a library's and a binary's documentation to the same `target/doc/<name>` directory, so
a bin target sharing a lib crate's name makes parallel rustdoc invocations race over
one path and the build dies. This family found that the hard way; every sibling now
separates its facade name from its command name.

The M0 namespace measurement ([M0 ground truth](../M0-ground-truth.md)) adds a second
constraint: on crates.io, `scankit` is already taken — since 2026-04-27, by an
unrelated directory-walking library (v0.3.0, 498 downloads, seemingly dormant) — while
`scankit-core`, `scankit-escl`, `scankit-wsd`, `scankit-http`, `scankit-discover`,
`scankit-testkit`, `scankit-cli`, and `scankit-driverkit` returned 404 (free) on
2026-08-11.

## Decision

- The workspace, repository, and facade crate are **`scankit`**; every family crate
  uses the **`scankit-`** prefix.
- The CLI binary is **`skan`** (owned by `scankit-cli`); the driver-work binary is
  **`scandev`** (owned by `scankit-driverkit`). Neither shares a name with any lib
  crate, and neither carries the family prefix, so no future crate can collide with a
  command.
- The crates.io *publication* name for the facade is a first-release decision, not a
  bootstrap one. The recorded options, in preference order: request the dormant
  `scankit` name through the crates.io transfer conversation; publish the facade under
  a sibling name while keeping this repository and API named scankit; or rename the
  family. Nothing in this workspace hard-codes the publication name — release-plz.toml
  carries the same note — so the decision stays open until it must close.

The gate is mechanical, not cultural. `just bin-name` reads every manifest and fails
on any bin target whose name matches a lib crate's, and it transcribes the two frozen
command names, so renaming a binary is an edit to the gate's table and to this ADR,
not a drive-by.

## Consequences

`skan` is two keystrokes shorter than `scanimage` and does not collide with anything
in common PATH inventories that M0 turned up; `scandev` reads as what it is and stays
out of end-user completion space.

The occupied facade name is the one real liability, and it is contained: it affects
`cargo add scankit` on the day of first publication and nothing before. The eight free
family names are the evidence that the prefix itself is safe; only the bare word is
contested. If the transfer conversation fails, the fallback costs a README line
("published as X, use `scankit` paths in code"), because Rust's package-rename feature
lets the dependency name and the lib name differ.

The bin-name gate was written and green at bootstrap, before either binary did
anything worth naming.
