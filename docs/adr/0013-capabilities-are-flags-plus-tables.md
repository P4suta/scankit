# ADR-0013: capabilities are flags plus tables

- Status: accepted
- Date: 2026-08-11

## Context

A scanner's capability document answers two different kinds of question, and models
that merge them answer both badly:

- **Does this device do X at all?** Duplex, color, a feeder, a flatbed — booleans a UI
  needs in one glance to draw its controls.
- **What exactly can source S do?** The platen supports 75–1200 DPI at three color
  modes; the ADF supports 300 DPI duplex but only in grayscale past 600. These are
  per-input-source value ranges, and they interact: "duplex" is a property of the ADF,
  not of the scanner, and eSCL's own `ScannerCapabilities` XML is structured that way.

SANE flattens both into one option list, which is why every frontend contains
heuristic re-derivation. A single struct-of-Options does no better. And an external
bitflags macro crate would put a third-party dependency into `scankit-core`, which
ADR 0003 forbids.

## Decision

`Capabilities` in `scankit-core` is two layers:

1. **`ScannerFeature` bit flags, hand-rolled.** A `u32`-backed set type with `const`
   flag values, written out by hand (no macro crate). Beside it lives `FLAGS`: the
   registry slice pairing every flag with its name, so iteration, display, and
   completeness checks are data-driven. A `const` assertion holds the registry and the
   flag definitions equal — bit count matches registry length, no bit repeats, no bit
   is skipped — so the compiler refuses a flag added to one side only.
2. **Per-source tables.** For each input source (platen, ADF simplex, ADF duplex) a
   table of what that source offers: resolution ranges, color modes, media geometry.
   The ticket validator (ADR 0010) reads these tables, never the flags.

The flags summarize; the tables decide. A flag never carries a value, and a table is
never consulted for a yes/no the flags answer.

The gate is mechanical, not cultural: the `const` assertion *is* a compiler-run gate —
a mismatched registry fails `cargo build` on every machine — and `just test` carries
the registry's completeness properties from M1. `just deps` keeps the macro crate this
design deliberately does without from arriving later as a refactor.

## Consequences

Hand-rolled flags cost perhaps sixty lines once, against a dependency in the one crate
that must have none. The registry-plus-assert pattern makes the flag set behave like a
derived enum without the macro: exhaustive iteration for UIs, name lookup for `skan
info`, and a compile error when the two drift.

The two-layer split means capability interpretation in the protocol cores has a
defined target shape: `scankit-escl` translates `ScannerCapabilities` XML into tables
per source and derives the flags from the tables, never the reverse, so a flag can
never claim what no table substantiates.

Devices whose XML contradicts itself (a duplex flag with no duplex table) surface at
translation time as a structured interoperability error rather than as a UI that
offers a mode the validator then refuses — the same honesty rule as ADR 0010, applied
one layer down.

The const assertion and the registry shape were frozen here at bootstrap, before the
first flag was defined.
