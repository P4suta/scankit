# Roadmap

Each milestone is independently verifiable, and none before M3 needs hardware: the
virtual device and the scripted transport are the primary test ground
([ADR 0011](docs/adr/0011-virtual-first-verification.md)).

## M0 — Measure (done)

Ground truth before design: incumbent SLOC (sane-backends, sane-airscan), dependency
cost of every candidate crate, crates.io namespace, and specification availability.
Results: [docs/M0-ground-truth.md](docs/M0-ground-truth.md). The fifteen decision
records in [docs/adr/](docs/adr/) cite these numbers rather than impressions.

## M1 — Vocabulary and the virtual device

`scankit-core` implemented in full: the traits, `ScanTicket`/`Capabilities` with the
const-asserted flag registry, the error vocabulary, the exchange types.
`scankit-backend-virtual` implements the traits against `Scenario` scripts, and the
facade's `Composite` carries it. The ticket validator and `nearest()` land here with
their mismatch table ([ADR 0010](docs/adr/0010-no-silent-negotiation.md)).

**Exit criterion:** `skan scan --backend virtual` produces a multi-page fixture from
an ADF scenario script — several pages, correct order, correct declared formats — and
the jam/cover-open/cancel scenarios surface as the right `Intervention` errors.

## M2 — eSCL against the mock

`scankit-escl` in full: capabilities XML interpretation, `ScanSettings` generation,
the job state machine. `scankit-http` (ureq + rustls + `TrustPolicy`) and
`scankit-backend-escl` pump it. Verified against `ScriptedHttp` and the testkit's
`std::net` mock server.

**Exit criterion:** the complete eSCL job lifecycle — capabilities, create, poll,
retrieve pages, completion, plus cancellation and every staged fault — runs green
against the mock, offline, in `cargo test`.

## M3 — Discovery and a real device

`scankit-discover` (mDNS/DNS-SD for `_uscan._tcp`/`_uscans._tcp`, WS-Discovery over
UDP with the codec from `scankit-wsd`), and `scandev` learns to capture cassettes.
First confirmatory hardware runs, marked `HW-verified`, never in required CI.

**Exit criterion:** `skan scan` discovers a scanner on a real LAN and lands one page
from it; the session is captured as a cassette that replays green offline.

## M4 — WSD

`scankit-wsd`'s WS-Scan model in full and `scankit-backend-wsd` pumping it, against
archived specification pages and cassettes; sane-airscan as the behavior oracle where
the text has decayed ([ADR 0012](docs/adr/0012-sane-airscan-is-the-behavior-oracle.md)).

**Exit criterion:** the WSD lifecycle runs green against scripted traffic, and a WSD
device scans via `skan` where hardware is available (confirmatory).

## M5 — Facade DevEx freeze and the 0.1 decision

The one-call API polished against real consumers, documentation complete, the
crates.io naming question closed ([ADR 0015](docs/adr/0015-naming.md)), and the
decision whether the surface is ready to be called 0.1 — releasing remains a
deliberate act, never automation ([ADR 0014](docs/adr/0014-workspace-register.md)).

## Non-goals

SANE API compatibility and a C ABI ([ADR 0002](docs/adr/0002-a-rust-api-not-a-sane-clone.md)),
image decoding and processing ([ADR 0007](docs/adr/0007-bytes-not-images.md)), and
USB-legacy or vendor-proprietary protocols
([ADR 0001](docs/adr/0001-driverless-first-no-usb-legacy.md)) are out of scope — the
first two permanently, the third for 1.0.
