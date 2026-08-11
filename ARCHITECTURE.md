# Architecture

Invariants only. Reasoning lives in [`docs/adr/`](docs/adr/).

## Shape

```text
        scankit-cli (skan)      scankit-driverkit (scandev)
                 ▲                  ▲
                 └────── scankit ───┘        ← facade: re-exports, Composite, one-call API
             ▲          ▲          ▲       ▲
  backend-escl  backend-wsd  backend-virtual  scankit-discover   ← pumps + discovery: own time and sockets' use
      ▲    ▲       ▲    ▲          ▲               ▲
  escl   http   wsd   http        core         wsd, core         ← sans-I/O cores + transport leaf
      ▲            ▲                               
     core         core                            
                                            scankit-core          ← the shared vocabulary, depends on nothing
```

`scankit-driverkit` may additionally reach past the facade into the protocol and
transport crates — the one consumer allowed to, because its job is bring-up. The
`scankit-testkit` harness sits outside the shipped graph entirely: dev-only, reachable
from every crate's tests except `scankit-core`'s (whose tests it would cycle with).

## Invariants

1. **Driverless first; no USB legacy** ([ADR 0001](docs/adr/0001-driverless-first-no-usb-legacy.md)).
   eSCL and WSD are the product. The core traits do not exclude a future USB backend
   seat; nothing else about one exists.

2. **A Rust API, not a SANE clone** ([ADR 0002](docs/adr/0002-a-rust-api-not-a-sane-clone.md)).
   SANE API compatibility and a C ABI are permanent non-goals. sane-airscan is a
   behavior reference, never an interface reference.

3. **The protocol cores are sans-I/O** ([ADR 0003](docs/adr/0003-sans-io-protocol-cores.md)).
   `scankit-escl` and `scankit-wsd` hold state machines over value types; sockets live
   in the two leaves, time lives in the pumps. `just deps` and `just purity` enforce it.

4. **Native `async fn`, static dispatch, no runtime** ([ADR 0004](docs/adr/0004-afit-static-dispatch-runtime-agnostic.md)).
   No `dyn`, no executor dependency in the default path.

5. **One job borrows the scanner** ([ADR 0005](docs/adr/0005-one-job-borrows-the-scanner.md)).
   `&mut self` everywhere; `Job<'a>` pins the device. Dropping a job abandons it
   lazily; `cancel(self)` is the guaranteed path.

6. **The error vocabulary is flat** ([ADR 0006](docs/adr/0006-flat-error-vocabulary.md)).
   One closed `#[non_exhaustive]` enum, translated at boundaries, no source chains.
   User-fixable failures are one `Intervention(InterventionReason)` class.

7. **Bytes, not images** ([ADR 0007](docs/adr/0007-bytes-not-images.md)). A page is the
   device's bytes plus its declared format. Decoding and PDF assembly are out of scope;
   only the `scankit-image` seat is reserved.

8. **HTTP is a leaf** ([ADR 0008](docs/adr/0008-http-client-is-a-leaf.md)). One crate
   owns the client (plan: ureq v3 + rustls, thread-bridged); the exchange value types
   make it replaceable.

9. **TLS trust is explicit** ([ADR 0009](docs/adr/0009-tls-trust-is-explicit.md)).
   `TrustPolicy` is `SystemRoots`, `TrustOnFirstUse`, or `Fingerprint`. No accept-all,
   no permissive default.

10. **No silent negotiation** ([ADR 0010](docs/adr/0010-no-silent-negotiation.md)).
    A ticket the device cannot honor is a structured error; rounding happens only in an
    explicit `nearest()` call.

11. **Virtual first, hardware confirmatory** ([ADR 0011](docs/adr/0011-virtual-first-verification.md)).
    The virtual backend, `ScriptedHttp`, and cassettes are the primary verification
    ground; hardware runs are marked and never required in CI.

12. **sane-airscan is the behavior oracle** ([ADR 0012](docs/adr/0012-sane-airscan-is-the-behavior-oracle.md)).
    Quirk decisions cite it with a pinned source; deviating from it requires an ADR
    appendix.

13. **Capabilities are flags plus tables** ([ADR 0013](docs/adr/0013-capabilities-are-flags-plus-tables.md)).
    Hand-rolled bit flags with a const-asserted registry, and per-input-source tables.

14. **The workspace register is frozen** ([ADR 0014](docs/adr/0014-workspace-register.md)).
    Edition 2024, rust-version 1.85, resolver 3, version 0.0.0, release automation all
    off, and the gate set below.

15. **Names are frozen** ([ADR 0015](docs/adr/0015-naming.md)). Facade `scankit`,
    family prefix `scankit-*`, binaries `skan` and `scandev`. `just bin-name` enforces
    the no-collision rule.

## What is gated

| Gate | Enforces | Runs |
| --- | --- | --- |
| `just deps` | every inter-crate edge is in the frozen ALLOWED matrix | offline, pre-commit |
| `just purity` | no socket, clock, or third party in the sans-I/O crates | offline, pre-commit |
| `just placeholder` | no `todo!`/`unimplemented!` body, no `allow`/`expect` | offline, pre-commit |
| `just unsafe-boundary` | no `unsafe` outside the (empty) allowlist | offline, pre-commit |
| `just bin-name` | no bin target shares a lib crate's name | offline, pre-commit |

Invariants 5, 6, 7, and 10 have no dedicated gate. Invariant 5 is enforced by the
borrow checker at every call site; 6, 7, and 10 rest on the frozen API shape, review,
and the M1/M2 test suites. Invariant 13's registry consistency is a `const` assertion,
which is a gate the compiler runs.

## Crate boundaries

| Crate | May depend on (direct intent) | Role |
| --- | --- | --- |
| `scankit-core` | — | vocabulary: traits, tickets, capabilities, errors, exchange types |
| `scankit-testkit` | `scankit-core` | dev-only harness: executor, `ScriptedHttp`, cassettes |
| `scankit-escl` | `scankit-core` | sans-I/O eSCL state machine |
| `scankit-wsd` | `scankit-core` | sans-I/O WS-Scan model + WS-Discovery codec |
| `scankit-http` | `scankit-core` | HTTP + TLS transport leaf, `TrustPolicy` |
| `scankit-discover` | `scankit-core`, `scankit-wsd` | mDNS/DNS-SD + WS-Discovery sockets |
| `scankit-backend-escl` | core, escl, http | the eSCL pump: retries, timeouts, polling |
| `scankit-backend-wsd` | core, wsd, http | the WSD pump |
| `scankit-backend-virtual` | `scankit-core` | scripted virtual device |
| `scankit` | core, discover, backends | facade: Composite, one-call API |
| `scankit-cli` | `scankit` | the `skan` command |
| `scankit-driverkit` | scankit, escl, wsd, http, discover | the `scandev` bring-up tool |
| `xtask` | — | the gates |

The table above records *direct intent*; the ALLOWED matrix in `xtask/src/deps.rs` is
its transitive closure and is what the `deps` gate checks manifests against. The two
absent edges everything leans on: neither protocol core reaches `scankit-http`, and
neither reaches the other.

## What lives where

- **Grammar lives in the protocol cores.** If XML, SOAP, or a datagram layout is
  parsed or rendered outside `scankit-escl`/`scankit-wsd`, it is in the wrong crate.
- **Time lives in the pumps.** The cores cannot name a clock (`just purity`), so every
  retry, timeout, and polling decision is a backend concern by construction.
- **Sockets live in the leaves.** `scankit-http` for TCP/TLS, `scankit-discover` for
  UDP and mDNS. Two crates touch the network; eleven do not.
- **Policy lives in the facade.** Which backend drives a device that offers both
  protocols, and what a one-call scan does, is composition — not something a protocol
  core may decide.
