# Architecture Decision Records

One record per architectural decision. Each states the context, the decision, and its
consequences as they stand now; superseding a decision adds a new record and marks the
old one superseded. `ARCHITECTURE.md` links here from its invariants list.

Format: numbered file, a `Status` (`proposed` / `accepted` / `superseded`), and a date.
Every Decision section names the mechanical gate that holds it — the gates were written
before the code they govern.

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-driverless-first-no-usb-legacy.md) | Driverless first: eSCL and WSD only, no USB legacy in 1.0 | accepted |
| [0002](0002-a-rust-api-not-a-sane-clone.md) | The product is the Rust API; SANE compatibility and C ABI are permanent non-goals | accepted |
| [0003](0003-sans-io-protocol-cores.md) | The protocol cores are sans-I/O; sockets in two leaves, time in the pumps | accepted |
| [0004](0004-afit-static-dispatch-runtime-agnostic.md) | Native `async fn` in traits, static dispatch, no runtime dependency | accepted |
| [0005](0005-one-job-borrows-the-scanner.md) | `&mut self` everywhere; a job borrows the scanner; cancel is explicit, drop is lazy | accepted |
| [0006](0006-flat-error-vocabulary.md) | One flat `#[non_exhaustive]` error vocabulary; `Intervention` is its own class | accepted |
| [0007](0007-bytes-not-images.md) | Pages are the device's bytes plus declared format; no image processing | accepted |
| [0008](0008-http-client-is-a-leaf.md) | HTTP lives in one leaf (plan: ureq + rustls, thread-bridged); replaceable by construction | accepted |
| [0009](0009-tls-trust-is-explicit.md) | TLS trust is a three-variant `TrustPolicy`; no accept-all, no default | accepted |
| [0010](0010-no-silent-negotiation.md) | Ticket/capability mismatch is a structured error; rounding only via explicit `nearest()` | accepted |
| [0011](0011-virtual-first-verification.md) | Virtual device, scripted HTTP, and cassettes are primary; hardware is confirmatory | accepted |
| [0012](0012-sane-airscan-is-the-behavior-oracle.md) | sane-airscan decides quirk questions, cited with pins; divergence is an ADR appendix | accepted |
| [0013](0013-capabilities-are-flags-plus-tables.md) | Capabilities are hand-rolled flags (const-asserted registry) plus per-source tables | accepted |
| [0014](0014-workspace-register.md) | The family workspace register: toolchain, lints, release posture, gates | accepted |
| [0015](0015-naming.md) | Facade `scankit`, prefix `scankit-*`, binaries `skan` and `scandev` | accepted |
