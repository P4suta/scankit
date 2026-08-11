# ADR-0008: the HTTP client is a leaf

- Status: accepted
- Date: 2026-08-11

## Context

Everything scankit says to a scanner travels over HTTP, so the choice of client is the
biggest single dependency decision in the workspace. M0 measured the candidates on
this machine (Windows, x86_64-pc-windows-msvc, 2026-08-11;
[M0 ground truth](../M0-ground-truth.md)):

- **ureq 3.4.0**, default features (rustls 0.23 + ring + gzip): **26 unique crates**,
  synchronous, TLS included, no runtime.
- **hyper 1.11.0** (http1, client) + http + http-body-util: **15 unique crates**, but
  tokio arrives as a required dependency, and a usable client still needs hyper-util
  and tokio's rt/net features — the honest count is higher and brings an executor.
- **quick-xml 0.41.0**: 2 crates (itself and memchr) — the XML side is free.

The real choice is therefore "synchronous and complete at 26" versus "async and
runtime-coupled at 15-plus". ADR 0004 already refused to impose a runtime, which
decides it: a synchronous client bridged onto the async surface by a worker thread
costs one thread per in-flight call — trivial at scanner concurrency (a handful of
devices, one job each) — and keeps every consumer's executor out of the picture.

sane-airscan's answer to the same question was to vendor `http_parser.c` (1,923 lines,
its largest file). That is the outcome a leaf crate exists to prevent.

## Decision

HTTP lives in exactly one crate, `scankit-http`. The day-one implementation plan is
ureq v3 with rustls, driven from a thread bridge that presents the stack's native
`async fn` surface. No other crate may name an HTTP client: the protocol cores speak
`HttpCall`/`HttpReply` values (ADR 0003), and the pumps hand those to this leaf.

Because every caller goes through the exchange value types, this is **not a one-way
door**: replacing ureq with a hyper stack (or anything else) is an implementation
change inside one crate, invisible in every signature above it. The decision is made
for 0.x on measurement and can be re-measured.

The gate is mechanical, not cultural. `just deps` gives only `scankit-http` a seat
above the transport (no other row reaches an HTTP crate), and `just purity` rejects
any HTTP client named inside the protocol cores.

## Consequences

The 26-crate footprint is the number a security-conscious adopter reviews, and it is
recorded in the M0 document with its measurement method so a future bump is a diff
against a baseline rather than a feeling.

The thread bridge means blocking I/O exists inside the leaf. That is contained: the
async surface above it never blocks an executor thread, which is the property ADR 0004
promised. If scanner concurrency assumptions ever change (mass fleet scanning), the
measured alternative is already documented in this file.

HTTP/2 is absent from ureq's default path and from every scanner this project has
evidence of; if a device ever requires it, that is an interoperability report first
and a leaf-internal change second.

The gates that isolate the leaf were turned on at bootstrap, before the leaf had any
implementation to isolate.
