# Architecture

This file records current invariants.
The trade-off that produced this shape is [ADR 0016](docs/adr/0016-one-product-crate.md).

## Product boundary

The workspace contains exactly two packages:

```text
scan-kit                         xtask
├── lib: scankit                └── repository gates (not product code)
└── bin: skan
```

Only `scan-kit` ships product code.
Creating a crate for an implementation layer is an architecture change: crates.io packages create names, dependency choices, and SemVer surfaces that this library does not need.

The library root has private modules and individual public re-exports:

```text
public facade: lib.rs
        │
        ▼
api ───────────────► driver ───────────────► transport
 │                      │                       │
 │                      ▼                       │
 └──────► model ◄── protocol ◄──── private wire values
             ▲          │
             └── error ◄┘
```

Dependency direction is downward/rightward.
In particular, `model`, `error`, `wire`,
and `protocol` cannot name `api`, `driver`, or `transport`.
Protocol code cannot name I/O traits, sockets, threads, or clocks.
`just purity` checks those absences over the module tree; Cargo packages are not required to obtain the boundary.

## Public surface

The root exports domain vocabulary, not implementation mechanisms:

| Area | Exported names |
| --- | --- |
| Find/connect | `discover`, `Device`, `ConnectOptions`, `Scanner`, `Fingerprint` |
| Job/stream | `Scan`, `Page`, `ScanOptions`, `ScanPlan` |
| Capabilities | `Capabilities`, `Source`, `Sides`, `ColorMode`, `Resolution`, `Region` |
| Failure | `Result`, opaque `Error`, `ErrorKind`, `Intervention` |

There are no public protocol types, HTTP values, driver traits, async-runtime choices,
backend enums, dependency re-exports, or feature flags.
`unreachable_pub = "deny"` makes every implementation item either private or deliberately reachable from the root.

The API is synchronous because the selected transports and physical scanner lifecycle are synchronous.
Applications that need asynchronous orchestration can place these blocking operations on their own blocking pool without scankit choosing an executor or multiplying its API.

## Lifecycle invariants

1. `Scanner::scan(&mut self)` returns `Scan<'_>`, so one scanner cannot carry two jobs.
2. `Scan::next_page(&mut self)` returns `Page<'_>`, so no next-page or cancel operation can overlap an open stream.
3. `Page` implements `std::io::Read`; bytes are never decoded or assembled.
4. A page must reach EOF.
   Dropping it early poisons that job except for cancellation.
5. `Scan::cancel(self)` is explicit and fallible.
   Dropping an unfinished scan stores the job for lazy cancellation before the next scan begins.
6. Control responses are bounded; page bodies remain streaming and unbounded.

## Capability policy

Capabilities are normalized per physical source.
An explicit option is an exact constraint and is never rounded.
Omitted settings resolve deterministically:

1. flatbed, then feeder;
2. one-sided;
3. color, then grayscale, then monochrome;
4. exactly 300 dpi, otherwise the greatest resolution not above 300 dpi, otherwise the smallest advertised resolution;
5. JPEG, then PDF, then the lexicographically first advertised MIME type;
6. the source's maximum region.

The exact choices are returned as `ScanPlan` before the protocol request is rendered.

## Protocol and transport boundary

`protocol::{escl,wsd,ws_discovery,xml}` parses bytes and renders private `HttpCall` values.
It owns no client, socket, timeout, retry, TLS, or discovery loop.
Synthetic request/reply tests therefore exercise protocol decisions without a network.

`transport::http` owns ureq, rustls, limits, and TLS identity.
`transport::discovery` owns mDNS and the WS-Discovery UDP socket.
`driver` is the only layer that combines a protocol with HTTP and translates private replies into canonical pages/errors.

WSD multipart image responses are peeled as a streaming reader: the SOAP control part is bounded, the binary part is not buffered, and its trailing MIME boundary is never leaked into page bytes.

## Discovery and trust

- mDNS browses `_uscan._tcp.local.` and `_uscans._tcp.local.`.
- WS-Discovery probes the IPv4 link-local multicast address and parses advertised scanner endpoints.
- Exact advertised identities merge endpoints; protocols never appear publicly.
- Secure endpoints sort first.
  If any HTTPS endpoint is advertised for a merged device, HTTP candidates are excluded rather than used as a downgrade.
- HTTPS uses platform verification by default.
  Explicit pinning compares the SHA-256 leaf fingerprint for every request.
  A mismatch is `UntrustedPeer`.
- Redirects are disabled, keeping authority changes explicit.

## Mechanical gates

| Gate | Invariant |
| --- | --- |
| Rust privacy + `unreachable_pub` | implementation modules do not become API |
| `deps` | exactly one product package plus `xtask`, with no workspace edges |
| `purity` | lower modules reach no upper module, I/O, socket, or clock |
| `placeholder` | no placeholder bodies or lint suppression attributes |
| `unsafe-boundary` | no unsafe token outside the empty allowlist |
| `bin-name` | `skan` exists and cannot collide with lib target `scankit` |
| strict Clippy | parser panic/arithmetic and API quality lints are warnings-as-errors |

## Permanent non-goals

SANE compatibility, a C ABI, image decoding, PDF assembly, public driver injection,
USB legacy drivers, vendor-proprietary protocols, and exposing transport dependencies are outside the architecture.
Supporting a new wire protocol is an internal change unless it requires new scanner-domain semantics.
