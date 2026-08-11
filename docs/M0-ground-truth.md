# M0 ground truth

The measurements the day-one design was frozen against, taken **2026-08-11** on
Windows 11 (x86_64-pc-windows-msvc). Numbers below are facts about that day and those
pinned revisions; anything re-measured later should be diffed against this file rather
than replacing it silently. Working artifacts lived in an ephemeral scratch directory
and are not preserved; everything needed to reproduce is recorded here.

## 1. Size of the incumbents (SLOC)

Tool: **tokei 14.0.0** (via `mise x tokei@latest`; not otherwise installed locally).
All numbers are tokei's *Code* column — comments and blanks excluded. Both repositories
cloned with `--depth 1`.

### sane-backends @ `f498f59` (2026-08-05)

| Scope | SLOC |
| --- | ---: |
| Whole repository, all languages | 538,883 |
| — of which C | 385,351 |
| — C headers | 40,320 |
| — C++ | 27,848 |
| — C family subtotal | 453,519 |
| — other (PO translations etc., largest: 78,457 PO) | rest |
| `backend/` directory, all languages | 407,208 |
| — C | 343,239 |
| — C headers | 35,291 |
| — C++ | 25,760 |
| — C family subtotal | 404,290 |

### sane-airscan @ `484ede7` (2026-08-03)

| Scope | SLOC |
| --- | ---: |
| Whole repository, all languages | 20,643 |
| — C | 18,118 |
| — C headers | 2,024 |

Module breakdown (C only):

| Module | SLOC | Note |
| --- | ---: | --- |
| `airscan-escl.c` | 957 | eSCL protocol |
| `airscan-wsd.c` | 878 | WS-Scan protocol |
| `airscan-wsdd.c` | 1,377 | WS-Discovery (pairs with wsd) |
| `airscan-zeroconf.c` | 1,032 | zeroconf model (pairs with mdns) |
| `airscan-mdns.c` | 972 | mDNS driver |
| `airscan-http.c` | 2,489 | HTTP transport |
| `http_parser.c` | 1,923 | **vendored** HTTP parser |
| `airscan-device.c` | 1,218 | device state machine |
| `airscan-devops.c` | 647 | device operations |
| `airscan-xml.c` | 706 | XML helpers |
| `test-*.c` (total) | 1,036 | unit tests |
| other C | remainder | |

Readings that shaped the design:

- **The driverless bet** (ADR 0001): the driver corpus is ~20x the size of the entire
  driverless stack, and the driverless stack alone covers most post-2015 devices.
- **Protocol cost comes in pairs**: WSD looks smaller than eSCL at the protocol file
  alone (878 vs 957), but WSD + its discovery half is 2,255 — protocol-vs-protocol
  comparisons must include the discovery pair (`airscan-zeroconf.c` likewise pairs
  with `airscan-mdns.c`). This is why `scankit-discover` and the codecs are split the
  way they are.
- **Vendoring pressure** (ADR 0008): sane-airscan's largest single file is a vendored
  HTTP parser. In Rust that entire file dissolves into `httparse`, already inside
  ureq's tree.

## 2. Dependency cost of the candidate crates

Method: `cargo new` an empty project, `cargo add` the one dependency, then count
`cargo tree -e normal --prefix none | sort -u` first-column crate names, excluding the
root. Measured on Windows (x86_64-pc-windows-msvc); platform-conditional dependencies
make some counts platform-specific.

| Candidate | Version | Unique crates | Notes |
| --- | --- | ---: | --- |
| ureq (default features) | 3.4.0 | 26 | rustls 0.23.43 + ring 0.17.14 + gzip included; sync, no runtime |
| quick-xml | 0.41.0 | 2 | itself + memchr — effectively free |
| mdns-sd | 0.20.3 | 15 | includes windows-sys/windows-link on this platform; Linux resolves differently |
| hyper (http1, client) + http + http-body-util | 1.11.0 / 1.5.0 / 0.1.4 | 15 | tokio 1.53.1 arrives as a required dependency; a practical client additionally needs hyper-util and tokio rt/net — the honest total is higher |

Conclusion material (consumed by ADR 0004 and ADR 0008): quick-xml is effectively
zero-cost, and the HTTP choice is "synchronous, complete, TLS included, 26 crates"
versus "15 crates plus a mandatory async runtime and further practical additions".

## 3. crates.io namespace (HTTP status of `/api/v1/crates/<name>`, 2026-08-11)

| Name | Status |
| --- | --- |
| `scankit` | **200 — taken** |
| `scankit-core` | 404 (free) |
| `scankit-escl` | 404 (free) |
| `scankit-wsd` | 404 (free) |
| `scankit-http` | 404 (free) |
| `scankit-discover` | 404 (free) |
| `scankit-testkit` | 404 (free) |
| `scankit-cli` | 404 (free) |
| `scankit-driverkit` | 404 (free) |

The existing `scankit`: v0.3.0, "Walk + watch + filter directory trees" — unrelated to
scanning hardware. Created 2026-04-27, 498 downloads, repository
`github.com/seryai/scankit`. Dormant-looking but the name is not usable. Consequence
and options are recorded in [ADR 0015](adr/0015-naming.md); the facade's publication
name is a first-release decision, and nothing in this workspace hard-codes it.

## 4. Specification availability

Checked live on 2026-08-11:

- **eSCL**: Mopria distributes the specification behind a click-through license at
  `https://mopria.org/spec-download` (HTTP 200; the direct
  `MopriaeSCLSpecDownload.php` link answers 403 without the click-through).
- **WSD**: Microsoft Learn is partially decaying. Alive:
  `.../image/web-services-on-devices-reference` (the hub),
  `.../image/scan-service--ws-scan`, and
  `.../image/wsd-scan-service-operation-elements` (all 200). Redirecting to the image
  index or gone (including from the MicrosoftDocs/windows-driver-docs repository):
  `scan-service--ws-scan--schema`, `wsd-scan-service-scanner-elements`,
  `scanner-description-elements`, `scanner-configuration-elements`.
  **Action standing**: archive (web.archive.org) any WSD page before implementing
  against it; `scankit-wsd` cites archived copies (its crate doc says so).

## 5. Reproducibility

- sane-backends: `f498f59`, cloned `--depth 1` (upstream dated 2026-08-05).
- sane-airscan: `484ede7`, cloned `--depth 1` (upstream dated 2026-08-03).
- Dependency probes: fresh `cargo new` projects per candidate, stable toolchain,
  2026-08-11, Windows x86_64-pc-windows-msvc.
- crates.io: `curl` with an explicit `-A` user agent against
  `https://crates.io/api/v1/crates/<name>`, reading the HTTP status.
