# Contributing

## Setup

```sh
mise install        # toolchain and gate tooling
mise run hooks      # install the git hooks
just                # list the available commands
```

## The loop

```sh
just check          # fast deterministic gates
just ci             # every gate, exactly as the pipeline runs it
```

`just ci` is also the pre-push hook. It runs offline; if it passes locally it passes in
CI. If it fails, fix the cause rather than narrowing the gate.

## Rules that are not negotiable

- **No `allow`, no `ignore`, no gate suppression.** Do not write `#[allow(...)]` or
  `#[expect(...)]`, do not add a word to the `typos` allowlist to make a spelling pass,
  and do not exclude a file from a check. Every gate is strict on purpose, and a
  suppression is invisible to the next reader in a way a failing build is not. The
  `placeholder` gate makes this mechanical: a suppression attribute in a product crate
  fails CI.

  The legitimate escape hatch is the **shared** configuration: `clippy.toml`, the
  `[workspace.lints]` table in `Cargo.toml`, `deny.toml`. If a lint is genuinely wrong
  for this codebase, change it there for the whole workspace, say why in the commit
  message, and leave the reason in a comment next to the setting. That turns one
  person's local exception into a decision the repository made, which is the entire
  difference.

  One setting already works that way and shows the shape: `doc-valid-idents` in
  `clippy.toml` lists the domain's proper nouns (eSCL, WS-Discovery, sane-airscan).
  **`doc-valid-idents` must keep `".."` as its last entry** — without it the list
  replaces Clippy's defaults instead of extending them.

- **There is one product package.** `scan-kit` contains every implementation layer;
  `xtask` is repository tooling.
  `just deps` rejects another workspace package or an edge between the two.
  A new crate is a publication and SemVer decision, not an organizational convenience ([ADR 0016](docs/adr/0016-one-product-crate.md)).

- **Private modules carry the architecture.** `model`, `error`, `wire`, and `protocol` may not reach `api`, `driver`, or `transport`; protocol code may not name I/O,
  sockets, threads, or clocks.
  `just purity` checks those upward edges.
  Put XML and SOAP interpretation in `protocol`, HTTP/TLS/mDNS in `transport`, their combination in `driver`, and caller policy in `api`.

- **The root is a curated SemVer boundary.** Do not write `pub mod`, expose transport dependencies, or add public traits/features for an internal extension point.
  Re-export a new domain name from `lib.rs` only after it has earned permanent support;
  `unreachable_pub = "deny"` catches accidental public implementation items.

- **Time belongs above protocols.** A retry loop, timeout, or polling interval written in protocol code fails `just purity`; render state/calls there and let transport or driver policy own the clock.

- **No unsafe.** The allowlist in `xtask/src/unsafe_boundary.rs` is empty, and an entry
  requires an ADR, a `// SAFETY:` comment on every block, and no `unsafe fn`. The
  workspace deliberately does not use a blanket `forbid(unsafe_code)`; the gate is
  stricter and survives the first legitimate exception.

- **Quirks cite the oracle.** A device-specific workaround names the sane-airscan
  source (file and commit) or the recorded traffic that motivated it
  ([ADR 0012](docs/adr/0012-sane-airscan-is-the-behavior-oracle.md)). An uncited quirk
  is an unexplained special case waiting to be deleted by someone it then breaks.

## Licensing every file you add

The repository is [REUSE](https://reuse.software/)-compliant and `just reuse` enforces it.
**Every source and config file you add opens with the same three lines**, in whatever
comment syntax the format uses:

```rust
// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0
```

```toml
# SPDX-FileCopyrightText: 2026 scankit contributors
#
# SPDX-License-Identifier: MIT OR Apache-2.0
```

Markdown documents and `.github/CODEOWNERS` are the exceptions: they carry no header and
are covered in bulk by `REUSE.toml`. If you add a file type that cannot carry a comment,
annotate it there rather than leaving it uncovered. Recorded device traffic is the one
case where the annotation is not boilerplate: a cassette names the device that produced
it and the terms it is shared under, in `REUSE.toml`, in the same change that adds it.

## Reading the reference is allowed

[sane-airscan](https://github.com/alexpevzner/sane-airscan) is GPL-2.0-or-later, and
this workspace is MIT or Apache-2.0. **Read it, run it, and cite it — never copy from
it.** Behavioral knowledge (which quirk a device needs, which XML field a firmware
omits) is fact, not expression, and recording it here with a citation is exactly what
[ADR 0012](docs/adr/0012-sane-airscan-is-the-behavior-oracle.md) prescribes. Code
structure, comments, and identifiers are expression; porting them across this license
boundary is not allowed. The eSCL and WSD specifications and this project's own
recorded traffic are the sources implementations are written from.

## Spelling

`typos` runs with `locale = "en-us"` and adding a word to its allowlist to silence it is
not an option. Write US spellings: `-or` not `-our`, `-ize` and `-ization` not their
`-is` forms, `-er` not `-re`, and a single `l` in past tenses such as `modeled`. Every
British form fails the gate. The only place an original spelling may stand is inside a
quoted title of a specification document.

The allowlist exists for a narrower thing: a name a protocol chose, such as `escl`, not
a word this project misspelled.

## Code and comments are in English

The repository, including comments and documentation, is written in concise English so
the spell checker works and so adopters can read it.

## Commits

Conventional Commits, validated by `committed` in the commit-msg hook:

```text
feat(escl): interpret ScannerCapabilities input-source tables
fix(discover): stop treating a ProbeMatch without XAddrs as fatal
docs(adr): record the TLS trust policy
```
