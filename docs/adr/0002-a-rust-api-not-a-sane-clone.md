# ADR-0002: a Rust API, not a SANE clone

- Status: accepted
- Date: 2026-08-11

## Context

The obvious adoption strategy for a new scanner stack is compatibility: implement the
SANE API, or export a C ABI that SANE frontends can load, and inherit every existing
frontend for free. It is worth stating why that strategy is refused, because it will be
suggested repeatedly.

The SANE API is a C interface from 1996: global initialization, integer option
descriptors mutated through `sane_control_option`, a blocking `sane_read` loop, and
device capabilities expressed as a flat option list that every frontend re-interprets
with its own heuristics. Every design decision this workspace freezes — capability
tables typed per input source (ADR 0013), one job borrowing the scanner (ADR 0005),
explicit TLS trust (ADR 0009), flat structured errors (ADR 0006) — is inexpressible
through that interface. A SANE-compatible scankit would be scankit reduced to SANE.

## Decision

The product is the Rust API. SANE API compatibility and a C ABI are permanent
non-goals, not deferred ones.

sane-airscan remains the *behavior* reference for device quirks (ADR 0012); it is
never an *interface* reference. No type, function name, or option model is carried
over for familiarity's sake.

If a C-consumable surface is ever wanted, it is a separate crate above the facade,
designed then, with its own ADR; nothing in this workspace reserves space for it.

The gate is mechanical, not cultural. A C ABI requires `extern "C"` and, in practice,
`unsafe`; `just unsafe-boundary` rejects the token `unsafe` everywhere (its allowlist
is empty), so an FFI surface cannot appear as a side effect of one enthusiastic pull
request — it would have to argue its way through this ADR first.

## Consequences

No existing SANE frontend can use scankit, so adoption must come from Rust
applications and from the `skan` CLI being good enough to use directly. That is a
slower path and a deliberate one: the hypothesis this project tests is that a modern
typed API is the missing piece, and shipping a compatibility shim first would leave
the hypothesis untested.

The DevEx bar is therefore the release bar: M5 exists specifically to freeze the
facade's ergonomics before 0.1 (ROADMAP.md), and API decisions get the scrutiny that
compatibility projects spend on conformance instead.

The unsafe-boundary gate was written and turned on at bootstrap, before any code
existed that could have wanted the exception.
