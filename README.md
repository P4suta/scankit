# scankit

A driverless-first scanner stack in pure Rust: discover a network scanner, read its
capabilities, and scan pages over eSCL or WSD — with a Rust-native API as the product.

> **Status: early development.** The workspace, the frozen architecture, and the quality
> gates are in place. **Nothing scans yet.** The design is recorded in fifteen decision
> records and the ground-truth measurements that motivated them; the first milestone
> with observable behavior is M1 ([ROADMAP.md](ROADMAP.md)).

## What this is

Since roughly 2015, network scanners have spoken two driverless protocols: eSCL (Mopria
"AirScan", what macOS uses) and WSD (what Windows uses). A scanner that speaks neither
is now the exception. The incumbent open-source stack reaches these devices through
SANE — 538,883 lines of C and a driver model designed for SCSI-era hardware — with the
driverless path grafted on as one backend, sane-airscan, which is 20,643 lines of C and
covers most modern devices by itself
([docs/M0-ground-truth.md](docs/M0-ground-truth.md)).

scankit is a bet on that measurement: implement the driverless pair natively in Rust,
skip the legacy driver model entirely, and make the developer experience of the Rust
API the product ([ADR 0001](docs/adr/0001-driverless-first-no-usb-legacy.md),
[ADR 0002](docs/adr/0002-a-rust-api-not-a-sane-clone.md)).

Three commitments shape everything else:

- **The protocol cores are sans-I/O.** `scankit-escl` and `scankit-wsd` are state
  machines over value types; they cannot name a socket or a clock, and a gate enforces
  it. Every protocol decision is testable from recorded traffic alone
  ([ADR 0003](docs/adr/0003-sans-io-protocol-cores.md)).
- **Verification is virtual first.** A scriptable virtual scanner — multi-page ADF
  runs, staged jams, a `hang()` hook — is the primary test ground; real hardware
  confirms, it never gates ([ADR 0011](docs/adr/0011-virtual-first-verification.md)).
- **Trust is explicit.** Scanners live on self-signed TLS; scankit makes the caller
  pick a `TrustPolicy` (system roots, trust-on-first-use, or a pinned fingerprint)
  instead of quietly accepting everything
  ([ADR 0009](docs/adr/0009-tls-trust-is-explicit.md)).

## Crates

| Crate | Responsibility | Network |
| --- | --- | --- |
| `scankit-core` | vocabulary: traits, tickets, capabilities, errors, exchange types | no |
| `scankit-escl` | sans-I/O eSCL protocol state machine | no |
| `scankit-wsd` | sans-I/O WS-Scan model + WS-Discovery codec | no |
| `scankit-http` | the HTTP/TLS transport leaf, `TrustPolicy` | yes |
| `scankit-discover` | mDNS/DNS-SD and WS-Discovery discovery leaf | yes |
| `scankit-backend-escl` | the eSCL pump: retries, timeouts, polling | via http |
| `scankit-backend-wsd` | the WSD pump | via http |
| `scankit-backend-virtual` | the scripted virtual device | no |
| `scankit` | the facade: re-exports, Composite, one-call API | via the above |
| `scankit-cli` | the `skan` command | via the facade |
| `scankit-driverkit` | the `scandev` bring-up tool (not published) | yes |
| `scankit-testkit` | dev-only test harness (not published) | loopback only |

The facade crate is `scankit` and the binary is `skan`; a bin target never shares a lib
crate's name ([ADR 0015](docs/adr/0015-naming.md)). Full shape and boundaries:
[ARCHITECTURE.md](ARCHITECTURE.md) and the [decision records](docs/adr/).

**A note on the crates.io name:** `scankit` on crates.io is occupied by an unrelated
directory-walking crate (measured 2026-08-11,
[docs/M0-ground-truth.md](docs/M0-ground-truth.md)). The other family names are free.
The publication name is a first-release decision; this repository and its API are not
hostage to it.

## Compared with sane-airscan

[sane-airscan][sane-airscan] is the proof that driverless-only coverage works, and this
project treats it as the behavior oracle for device quirks — cited with pinned sources,
never silently diverged from ([ADR 0012](docs/adr/0012-sane-airscan-is-the-behavior-oracle.md)).
What scankit does differently is structural, frozen on day one: a Rust API instead of a
SANE backend, sans-I/O cores instead of protocol code entangled with its event loop,
and a virtual-first test bed instead of hardware-dependent verification.

## Development

```sh
mise install        # toolchain and gate tooling
mise run hooks      # install the git hooks
just                # list the available commands
just check          # fast inner-loop gates
just ci             # every gate, exactly as the pipeline runs it
```

`just ci` runs offline and predicts CI exactly. The workspace-specific gates — `deps`,
`purity`, `placeholder`, `unsafe-boundary`, `bin-name` — live in `xtask` and are
documented in [ARCHITECTURE.md](ARCHITECTURE.md).

## License

Dual-licensed under [MIT](LICENSES/MIT.txt) or [Apache-2.0](LICENSES/Apache-2.0.txt), at
your option. The repository is [REUSE](https://reuse.software/)-compliant.

[sane-airscan]: https://github.com/alexpevzner/sane-airscan
