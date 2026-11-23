---
status: proposed
date: 2026-08-12
---
# One product package and private modules

## Context

The bootstrap workspace exposes internal implementation layers as separate Cargo packages.
The recovered scanner implementation uses a synchronous API and private protocol and transport modules.

## Decision

Keep one product package, `scan-kit`, with library `scankit` and command `skan`.
Keep `xtask` separate.
Expose scanner-domain values and borrowing scan/page operations.
Keep protocol, HTTP, discovery, and driver types private.
Acceptance would supersede the implementation choices in ADRs 0003–0006, 0008–0011, 0013, and 0015.

## Alternatives

The accepted multi-package design gives stronger compilation boundaries and an asynchronous API.
Retaining that design would require adapting the recovered implementation and maintaining a public contract for each package.

## Consequences

Private modules reduce public compatibility obligations but require dependency checks within the package.
Asynchronous consumers need a blocking pool.
Reconsider when a consumer needs concurrent scans or an independent transport package.

## Confirmation

`just deps` and `just purity` check package and module boundaries.
Strict Clippy, compile-fail tests, and loopback scanner tests check the public API and streaming lifecycle.
