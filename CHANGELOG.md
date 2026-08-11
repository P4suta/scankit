# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Workspace bootstrap: thirteen crate skeletons, the quality gates (`deps`, `purity`,
  `placeholder`, `unsafe-boundary`, `bin-name` in `xtask`, plus the shared
  format/lint/spell/license/supply-chain set), CI with a single `ci-required`
  aggregation gate, and the day-one architectural decision records. Nothing scans yet.
- `docs/M0-ground-truth.md`: the M0 measurements — incumbent SLOC (sane-backends,
  sane-airscan), dependency-tree costs of the candidate crates, crates.io name
  availability, and specification URL liveness — that the frozen design cites.
