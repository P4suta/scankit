# Project overview

scankit is an early stage Rust workspace for eSCL and Web Services on Devices (WSD) scanner support.
Scanner discovery and page scanning aren't implemented, and the `skan` command has no subcommands.

The project explores a Rust API for scanners that support eSCL or WSD.
The [M0 ground-truth measurements](M0-ground-truth.md) record the evidence behind that direction.
They include size and dependency measurements for existing scanner stacks.

scankit explores implementing the driverless protocols in Rust without adopting a legacy scanner driver model.
The [driverless-first decision](adr/0001-driverless-first-no-usb-legacy.md) and [Rust API decision](adr/0002-a-rust-api-not-a-sane-clone.md) record that direction.

The design keeps sockets and clocks outside the protocol cores.
The verification plan uses virtual devices.
The planned API requires callers to choose a TLS trust policy.
The [architecture overview](../ARCHITECTURE.md), [roadmap](../ROADMAP.md), and [decision records](adr/README.md) document the crate boundaries, planned milestones, and supporting decisions.

`just check` runs the fast inner-loop gates.
`just ci` runs all gates, including workspace tests, documentation, and Rust 1.85 verification.
