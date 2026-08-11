# ADR-0003: sans-I/O protocol cores

- Status: accepted
- Date: 2026-08-11

## Context

sane-airscan entangles protocol knowledge with its event loop: `airscan-escl.c` (957
lines) and `airscan-wsd.c` (878 lines) both drive their state machines by submitting
HTTP requests from inside protocol callbacks, which is why testing it means owning a
scanner and why its quirk fixes are hard to verify after the fact
([M0 ground truth](../M0-ground-truth.md)). It also vendors its own HTTP parser —
`http_parser.c`, 1,923 lines, the largest single file in the project — because the
protocol code needed one close at hand.

The alternative is the sans-I/O pattern: the protocol is a pure state machine over
value types — "given this state and this reply, here is the new state and the next
call to make" — and something else owns sockets, retries, and time. The cost is a
seam: every request/reply must be reified as a value. The benefit is that every
protocol decision becomes testable from recorded traffic alone.

## Decision

`scankit-escl` and `scankit-wsd` are sans-I/O. They speak entirely in the
`HttpCall`/`HttpReply`/`Datagram` value types owned by `scankit-core::exchange`. They
may not name a socket, observe a clock, spawn a thread, or take any third-party
dependency beyond `quick-xml` (measured at two resolved crates — effectively free).

Sockets live in exactly two leaves: `scankit-http` (TCP/TLS) and `scankit-discover`
(UDP/mDNS). Time — retries, timeouts, polling cadence — lives in the pumps,
`scankit-backend-escl` and `scankit-backend-wsd`, which is enforced rather than
conventional because a crate that cannot read a clock cannot own a timeout.

`scankit-core` itself is stricter still: zero dependencies of any kind.

The gate is mechanical, not cultural. `just purity` rejects sockets, clock reads,
threads, and unallowed third parties in the four pure crates; `just deps` holds every
inter-crate edge to the frozen ALLOWED matrix, whose two load-bearing absences —
neither protocol core reaches `scankit-http`, and neither reaches the other — are
asserted by the matrix's own tests.

## Consequences

Every protocol behavior, including every future device quirk, is expressible as a unit
test over value types: state in, reply in, state and next call out. The virtual-first
verification strategy (ADR 0011) exists because this ADR makes it possible.

The seam has a real cost: reifying exchanges as values means the pumps contain
translation boilerplate, and a contributor's first instinct — "just make the request
here" — fails the gate. CONTRIBUTING.md warns about this explicitly, because the gate
cannot be talked around and should not surprise anyone.

The cores are `std`, not `no_std`: they use ordinary strings and collections, and the
design claims I/O-freedom, not libc-freedom. This is why the family's usual `no-std`
and `wasm` Justfile lanes are deliberately absent (a comment in the Justfile records
this); a wasm lane returns if and when a consumer needs one, as a compile check rather
than a design change.

Both gates were written and turned on at bootstrap, before any protocol code existed
for them to fail on.
