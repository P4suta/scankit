# ADR-0006: a flat error vocabulary

- Status: accepted
- Date: 2026-08-11

## Context

Layered Rust stacks default to layered errors: the transport has an error enum, the
protocol wraps it, the backend wraps that, and the application receives
`ScanError(Backend(Protocol(Http(Io(...)))))` — a source chain the caller must walk to
answer the only questions that matter at a scanning UI: *can I retry, do I need the
user to do something at the device, or is this fatal?* Chains also leak lower-layer
types into upper-layer public APIs, which turns every transport swap into a semver
event.

Scanning has a class of failures no other layer of the stack can invent: the ADF is
empty, the paper jammed, the cover is open, the flatbed is busy warming up. These are
not errors in the software sense at all — they are requests for a human — and burying
them among socket resets guarantees every frontend re-derives the distinction with
string matching, which is what SANE frontends actually do.

## Decision

`scankit-core` owns one flat, closed error vocabulary: a single `#[non_exhaustive]`
enum with no `source()` chain. Every boundary *translates* into it — the eSCL pump
maps an HTTP 503 plus a job-state document into the vocabulary term it means — rather
than wrapping what it received. Diagnostic detail (status codes, fault strings) rides
inside the relevant variant as plain data, not as a nested error type.

User-serviceable conditions are one class, `Intervention(InterventionReason)`, with
`InterventionReason` covering at least `AdfEmpty`, `AdfJam`, `CoverOpen`. A frontend
matches one variant to know "show the user a message and offer retry"; it never
pattern-matches protocol details to reconstruct that meaning.

The gate is mechanical, not cultural. The vocabulary lives in `scankit-core`, whose
zero-dependency row (`just deps`, `just purity`) keeps `thiserror`-style derive stacks
and any wrapped foreign error type out by construction — a source chain cannot be
declared without naming a type the crate is forbidden to name. `#[non_exhaustive]`
makes rustc enforce forward-compatible matching in every consumer.

## Consequences

Translation is work: every backend author must decide, per failure, what it *means* in
the shared vocabulary, and a lazy `Other(String)` escape hatch is conspicuously absent
so that the decision cannot be deferred. Where the mapping is genuinely unknown (a
fault string no one has seen), the honest term is a dedicated variant carrying the raw
material, and its appearance in the wild is an interoperability report.

Losing `source()` means losing `anyhow`-style causal display for free. The traded gain
is that the public error surface never changes when the HTTP client does (ADR 0008),
and that match statements in frontends are exhaustive over meanings rather than over
plumbing.

The `Intervention` class is the piece expected to age best: it is the vocabulary a
scan UI is written in, and it exists as a type from day one.

The vocabulary's shape was frozen here at bootstrap, before the first translation into
it was written.
