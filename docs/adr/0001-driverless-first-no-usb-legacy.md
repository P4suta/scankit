# ADR-0001: driverless first, no USB legacy

- Status: accepted
- Date: 2026-08-11

## Context

The incumbent open-source scanner stack, SANE, is 538,883 lines measured at commit
f498f59, of which 407,208 live in `backend/` — per-device drivers for hardware
reaching back to SCSI scanners ([M0 ground truth](../M0-ground-truth.md)). Its
driverless path exists as one backend among dozens: sane-airscan, 20,643 lines of C,
which by its own coverage speaks for most scanners sold since roughly 2015, because
those scanners implement eSCL (Mopria "AirScan", the protocol macOS uses) or WSD (the
protocol Windows uses), usually both.

The measurement is the argument: the driverless fraction of the problem is two orders
of magnitude smaller than the driver-model fraction, and it is the fraction that grows.
Every AirPrint-certified multifunction device speaks eSCL; Windows certification pushes
WSD. Meanwhile the per-device driver corpus serves devices that age out of service.

A new stack must decide on day one whether it carries a seat for USB-legacy and
vendor-proprietary protocols, because that decision shapes the core trait: a transport
abstraction wide enough for "send this vendor blob over USB bulk transfer" is a
different and worse abstraction than one shaped for "exchange HTTP messages with a
device that describes itself".

## Decision

scankit supports eSCL and WSD, over IP, and nothing else. USB-legacy and
vendor-proprietary protocols are non-goals for 1.0.

One guarantee is retained on purpose: the `Backend`/`Scanner`/`ScanJob` traits in
`scankit-core` are expressed in terms of capabilities, tickets, jobs, and pages — not
in terms of HTTP — so a future `scankit-backend-usb` would be one more implementor of
the same traits. The trait shape must not exclude that seat; nothing else about it may
exist: no module, no enum variant, no reserved configuration.

The gate is mechanical, not cultural. `just deps` holds the crate graph to the ALLOWED
matrix in `xtask/src/deps.rs`, which contains no USB crate and no row for one; adding
the seat requires editing the frozen matrix, which is an ADR-visible act, and `just
placeholder` keeps a speculative half-implementation from standing in for the decision.

## Consequences

Devices without eSCL or WSD — pre-2015 hardware, film scanners, some sheet-fed
oddities — are out of scope, and users with them stay on SANE. That is accepted: the
alternative is inheriting the 407,208-line liability that makes the incumbent hard to
maintain.

In exchange, the whole stack fits in the shape this workspace freezes: two protocol
cores, two socket leaves, and pumps. Nothing in the codebase ever branches on "is this
a driver device", because the concept does not exist here.

The trait-shape guarantee costs review attention at M1, when the traits are
implemented: the reviewer must ask "does this signature assume HTTP" for every method.
The virtual backend (ADR 0011) is the standing proof, since it implements the full
trait surface with no network at all.

The gates named above were written and turned on at bootstrap, before any
implementation existed for them to fail on.
