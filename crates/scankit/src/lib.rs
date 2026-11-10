// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A small, synchronous, driverless scanner library.
//!
//! The public API deliberately contains scanner-domain values only.
//! eSCL, WSD,
//! discovery, HTTP, and TLS are private implementation details, so applications do not inherit their versioning or architecture.
//!
//! # Quick start
//!
//! ```no_run
//! use std::{fs::File, io, time::Duration};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let Some(device) = scankit::discover(Duration::from_secs(3))?
//!     .into_iter()
//!     .next()
//! else {
//!     return Ok(());
//! };
//!
//! let mut scanner = device.connect()?;
//! let mut scan = scanner.scan()?;
//!
//! while let Some(mut page) = scan.next_page()? {
//!     let extension = page.suggested_extension().unwrap_or("bin");
//!     let mut output = File::create(format!(
//!         "scan-{number:03}.{extension}",
//!         number = page.number(),
//!     ))?;
//!     io::copy(&mut page, &mut output)?;
//! }
//! # Ok(())
//! # }
//! ```

mod api;
mod driver;
mod error;
mod model;
mod protocol;
mod transport;
mod wire;

pub use api::{ConnectOptions, Device, Page, Scan, Scanner, discover};
pub use error::{Error, ErrorKind, Fingerprint, Intervention, Result};
pub use model::{
    Capabilities, ColorMode, Region, Resolution, ScanOptions, ScanPlan, Sides, Source,
};
