# ADR-0004: native `async fn` in traits, static dispatch, runtime-agnostic

- Status: accepted
- Date: 2026-08-11

## Context

Scanning is I/O with long waits: a page takes seconds, a job takes minutes, and
applications embedding this stack — a GUI, a service, a CLI — differ in how they want
to wait. The trait surface must be async, and the question is which flavor:

1. `async_trait`-style boxing: `dyn`-friendly, but every call allocates, every future
   is type-erased, and the crate exports a proc-macro dependency to every consumer.
2. Native `async fn` in traits (AFIT, stable since Rust 1.75) with static dispatch:
   zero-cost, no dependency, but no `dyn Backend` and therefore no trait-object
   heterogeneity.
3. Hand-rolled poll functions: maximally flexible, unwritable by normal contributors.

A second, orthogonal temptation is to adopt an async runtime (tokio) as a dependency
of the core, which would decide the executor question for every consumer of a library
whose actual concurrency need — a handful of concurrent HTTP calls — is modest.

## Decision

The `scankit-core` traits use native `async fn` with static dispatch. There is no
`dyn` in the public surface and no runtime dependency anywhere in the default path;
the M0 dependency measurements ([M0 ground truth](../M0-ground-truth.md)) showed a
hyper/tokio stack starts at 15 crates *plus* a mandatory runtime, against ureq's 26
with none, and the thread-bridge in `scankit-http` (ADR 0008) is what reconciles a
synchronous client with an async surface.

Heterogeneity — "a list of scanners, some eSCL, some WSD, some virtual" — is provided
by the `Composite` backend in the facade: an enum over the known backends, not a trait
object. Adding a backend is adding a variant, which is the closed-world assumption
ADR 0001 already made.

The gate is mechanical, not cultural. `just deps` keeps every runtime crate out of the
core (`scankit-core`'s row is empty and `just purity` rejects third parties in it),
and the testkit's own hand-rolled `block_on`/`poll_n` executor exists precisely so
that no test ever imports tokio to drive a future.

## Consequences

Consumers on any runtime — tokio, smol, pollster, a GUI event loop — can drive the
stack, because a future that only awaits other futures is runtime-neutral.

The composite-enum approach means third-party backends cannot be injected from outside
the facade. That is accepted for 1.0 and recorded here as the known cost; a
`Backend`-generic API remains available to consumers who want to drive one specific
backend statically.

AFIT futures are not `Send`-bounded by default; whether the facade constrains them is
an M5 DevEx decision to be made against real consumers, not guessed now.

The gates named above were written and turned on at bootstrap, before any trait
existed for them to protect.
