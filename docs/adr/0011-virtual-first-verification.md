# ADR-0011: virtual-first verification

- Status: accepted
- Date: 2026-08-11

## Context

Scanner software is traditionally verified against scanners, which is why it is
traditionally under-verified: hardware is scarce in CI, firmware varies by region and
revision, and a jam cannot be staged on demand. sane-airscan's test surface reflects
the constraint — of its 20,643 lines, its `test-*.c` files total 1,036, mostly unit
tests of codecs, with device behavior verified by maintainers who own the devices
([M0 ground truth](../M0-ground-truth.md)).

The sans-I/O decision (ADR 0003) removes the constraint. If every protocol step is a
function over value types, then a scripted counterparty is a complete test double, and
hardware is only needed to *discover* behavior, never to re-verify it.

## Decision

The primary verification ground is virtual, and it has three instruments, all dev-side:

- **`scankit-backend-virtual`**: a full trait implementation driven by a `Scenario` —
  a consuming builder scripting capabilities, multi-page ADF runs, staged
  interventions (jam after page two, cover open, empty feeder), and a `hang()` point
  where the device goes silent. It verifies everything above the traits: facade, CLI,
  composite, error mapping.
- **`ScriptedHttp`** in `scankit-testkit`: a transport double that answers each
  `HttpCall` from a script and fails on deviation. It verifies the pumps.
- **Cassettes**: recorded real-device exchanges, captured by `scandev`, replayed byte
  for byte. They pin interoperability fixes to the traffic that motivated them.

Real hardware is *confirmatory*: an M3 exit criterion runs against a LAN device, and
such runs are marked `HW-verified` in their test names and reports. No required CI job
touches hardware — `ci-required` aggregates only jobs a laptop can run offline.

The gate is mechanical, not cultural. `just test` runs the entire virtual bed in
ordinary `cargo test`; `just purity` keeps the virtual backend itself pure (a scripted
hang is a state, not a sleep), so the test bed can never quietly grow the flakiness it
exists to eliminate; and the `ci-required` needs-list in `.github/workflows/ci.yml` is
the machine-readable form of "hardware never gates".

## Consequences

A contributor without a scanner can develop, test, and fix everything except a brand
new device quirk — and with a cassette attached to the issue, the quirk too. This is
the property that makes outside contribution plausible at all.

The virtual device is a model, and models drift from reality. The discipline that
contains the drift: every quirk discovered on hardware lands as a cassette or a
scenario in the same commit as its fix, so the virtual bed monotonically approaches
the field. A fix without its recording is an unverifiable anecdote and gets reviewed
as one.

Cassettes carry device-produced data, so each one is annotated in `REUSE.toml` with
its origin and terms (CONTRIBUTING.md); a recording nobody may redistribute does not
land.

The instruments were specified at bootstrap, before the first line of behavior existed
for them to verify.
