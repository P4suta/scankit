// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use crate::error::{Error, ErrorKind, Result};

/// The physical input used for a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Source {
    /// The flat glass platen.
    Flatbed,
    /// The automatic document feeder.
    Feeder,
}

/// The number of sides read from each sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Sides {
    /// Read one side of each sheet.
    One,
    /// Read both sides of each sheet.
    Two,
}

/// The requested color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ColorMode {
    /// One bit per pixel.
    Monochrome,
    /// Grayscale pixels.
    Grayscale,
    /// Full-color pixels.
    Color,
}

/// Horizontal and vertical resolution in dots per inch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Resolution {
    x_dpi: u32,
    y_dpi: u32,
}

impl Resolution {
    /// Creates an anisotropic resolution.
    #[must_use]
    pub const fn new(x_dpi: u32, y_dpi: u32) -> Self {
        Self { x_dpi, y_dpi }
    }

    /// Creates a square resolution.
    #[must_use]
    pub const fn dpi(dpi: u32) -> Self {
        Self::new(dpi, dpi)
    }

    /// Returns the horizontal dots per inch.
    #[must_use]
    pub const fn x_dpi(self) -> u32 {
        self.x_dpi
    }

    /// Returns the vertical dots per inch.
    #[must_use]
    pub const fn y_dpi(self) -> u32 {
        self.y_dpi
    }

    const fn is_valid(self) -> bool {
        self.x_dpi != 0 && self.y_dpi != 0
    }
}

/// A rectangular scan area measured from the input's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Region {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Region {
    /// Creates a region using micrometers.
    #[must_use]
    pub const fn micrometers(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Creates a region using whole millimeters.
    ///
    /// Returns None when conversion would overflow.
    #[must_use]
    pub fn millimeters(x: u32, y: u32, width: u32, height: u32) -> Option<Self> {
        Some(Self::micrometers(
            x.checked_mul(1_000)?,
            y.checked_mul(1_000)?,
            width.checked_mul(1_000)?,
            height.checked_mul(1_000)?,
        ))
    }

    /// Returns the horizontal offset in micrometers.
    #[must_use]
    pub const fn x_micrometers(self) -> u32 {
        self.x
    }

    /// Returns the vertical offset in micrometers.
    #[must_use]
    pub const fn y_micrometers(self) -> u32 {
        self.y
    }

    /// Returns the width in micrometers.
    #[must_use]
    pub const fn width_micrometers(self) -> u32 {
        self.width
    }

    /// Returns the height in micrometers.
    #[must_use]
    pub const fn height_micrometers(self) -> u32 {
        self.height
    }

    fn fits_within(self, maximum: Self) -> bool {
        let Some(right) = self.x.checked_add(self.width) else {
            return false;
        };
        let Some(bottom) = self.y.checked_add(self.height) else {
            return false;
        };
        right <= maximum.width && bottom <= maximum.height && self.width != 0 && self.height != 0
    }
}

/// Optional constraints for one scan.
///
/// Every value explicitly set by the caller is exact.
/// Unspecified values use the documented conservative defaults and are recorded in the resulting `ScanPlan`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanOptions {
    pub(crate) source: Option<Source>,
    pub(crate) sides: Option<Sides>,
    pub(crate) color_mode: Option<ColorMode>,
    pub(crate) resolution: Option<Resolution>,
    pub(crate) format: Option<String>,
    pub(crate) region: Option<Region>,
}

impl ScanOptions {
    /// Creates an unconstrained scan request.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            source: None,
            sides: None,
            color_mode: None,
            resolution: None,
            format: None,
            region: None,
        }
    }

    /// Requires a particular input source.
    #[must_use]
    pub const fn source(mut self, source: Source) -> Self {
        self.source = Some(source);
        self
    }

    /// Requires one-sided or two-sided scanning.
    #[must_use]
    pub const fn sides(mut self, sides: Sides) -> Self {
        self.sides = Some(sides);
        self
    }

    /// Requires a particular color representation.
    #[must_use]
    pub const fn color_mode(mut self, color_mode: ColorMode) -> Self {
        self.color_mode = Some(color_mode);
        self
    }

    /// Requires a particular horizontal and vertical resolution.
    #[must_use]
    pub const fn resolution(mut self, resolution: Resolution) -> Self {
        self.resolution = Some(resolution);
        self
    }

    /// Requires a square resolution in dots per inch.
    #[must_use]
    pub const fn dpi(self, dpi: u32) -> Self {
        self.resolution(Resolution::dpi(dpi))
    }

    /// Requires a particular MIME media type.
    #[must_use]
    pub fn format(mut self, format: &str) -> Self {
        self.format = Some(format.trim().to_ascii_lowercase());
        self
    }

    /// Requires a particular scan area.
    #[must_use]
    pub const fn region(mut self, region: Region) -> Self {
        self.region = Some(region);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InputCapabilities {
    pub(crate) source: Source,
    pub(crate) sides: Vec<Sides>,
    pub(crate) color_modes: Vec<ColorMode>,
    pub(crate) resolutions: Vec<Resolution>,
    pub(crate) formats: Vec<String>,
    pub(crate) maximum_region: Region,
}

/// The normalized capabilities reported by a scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    inputs: Vec<InputCapabilities>,
}

impl Capabilities {
    pub(crate) fn new(inputs: Vec<InputCapabilities>) -> Result<Self> {
        if inputs.is_empty() {
            return Err(Error::new(
                ErrorKind::Protocol,
                "scanner advertised no usable input source",
            ));
        }
        Ok(Self { inputs })
    }

    /// Iterates over the scanner's input sources.
    pub fn sources(&self) -> impl ExactSizeIterator<Item = Source> + '_ {
        self.inputs.iter().map(|input| input.source)
    }

    /// Iterates over the side modes for one source.
    pub fn sides(&self, source: Source) -> impl Iterator<Item = Sides> + '_ {
        self.input(source)
            .into_iter()
            .flat_map(|input| input.sides.iter().copied())
    }

    /// Iterates over the color modes for one source.
    pub fn color_modes(&self, source: Source) -> impl Iterator<Item = ColorMode> + '_ {
        self.input(source)
            .into_iter()
            .flat_map(|input| input.color_modes.iter().copied())
    }

    /// Iterates over the resolutions for one source.
    pub fn resolutions(&self, source: Source) -> impl Iterator<Item = Resolution> + '_ {
        self.input(source)
            .into_iter()
            .flat_map(|input| input.resolutions.iter().copied())
    }

    /// Iterates over MIME media types for one source.
    pub fn formats(&self, source: Source) -> impl Iterator<Item = &str> + '_ {
        self.input(source)
            .into_iter()
            .flat_map(|input| input.formats.iter().map(String::as_str))
    }

    /// Returns the largest addressable region for a source.
    #[must_use]
    pub fn maximum_region(&self, source: Source) -> Option<Region> {
        self.input(source).map(|input| input.maximum_region)
    }

    /// Resolves options into the exact settings that would be sent to the scanner.
    pub fn resolve(&self, options: &ScanOptions) -> Result<ScanPlan> {
        let input = match options.source {
            Some(source) => self.input(source).ok_or_else(|| {
                Error::new(
                    ErrorKind::Unsupported,
                    format!("scanner does not provide the requested {source:?} source"),
                )
            })?,
            None => self.default_input()?,
        };

        let sides = exact_or_default(
            options.sides,
            &input.sides,
            &[Sides::One, Sides::Two],
            "side mode",
        )?;
        let color_mode = exact_or_default(
            options.color_mode,
            &input.color_modes,
            &[
                ColorMode::Color,
                ColorMode::Grayscale,
                ColorMode::Monochrome,
            ],
            "color mode",
        )?;
        let resolution = resolve_resolution(options.resolution, &input.resolutions)?;
        let format = resolve_format(options.format.as_deref(), &input.formats)?;
        let region = options.region.unwrap_or(input.maximum_region);
        if !region.fits_within(input.maximum_region) {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "requested scan region lies outside the selected input",
            ));
        }

        Ok(ScanPlan {
            source: input.source,
            sides,
            color_mode,
            resolution,
            format,
            region,
        })
    }

    fn input(&self, source: Source) -> Option<&InputCapabilities> {
        self.inputs.iter().find(|input| input.source == source)
    }

    fn default_input(&self) -> Result<&InputCapabilities> {
        self.input(Source::Flatbed)
            .or_else(|| self.input(Source::Feeder))
            .or_else(|| self.inputs.first())
            .ok_or_else(|| Error::new(ErrorKind::Protocol, "scanner has no input source"))
    }
}

fn exact_or_default<T>(
    requested: Option<T>,
    supported: &[T],
    preferences: &[T],
    setting: &str,
) -> Result<T>
where
    T: Copy + PartialEq + fmt::Debug,
{
    if let Some(requested) = requested {
        return supported
            .contains(&requested)
            .then_some(requested)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Unsupported,
                    format!("scanner does not support requested {setting} {requested:?}"),
                )
            });
    }

    preferences
        .iter()
        .copied()
        .find(|candidate| supported.contains(candidate))
        .or_else(|| supported.first().copied())
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Protocol,
                format!("scanner advertised no usable {setting}"),
            )
        })
}

fn resolve_resolution(
    requested: Option<Resolution>,
    supported: &[Resolution],
) -> Result<Resolution> {
    if let Some(requested) = requested {
        if !requested.is_valid() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "resolution must be greater than zero",
            ));
        }
        return supported
            .contains(&requested)
            .then_some(requested)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Unsupported,
                    format!("scanner does not support requested resolution {requested:?}"),
                )
            });
    }

    supported
        .iter()
        .copied()
        .find(|resolution| *resolution == Resolution::dpi(300))
        .or_else(|| {
            supported
                .iter()
                .copied()
                .filter(|resolution| resolution.x_dpi <= 300 && resolution.y_dpi <= 300)
                .max_by_key(|resolution| (resolution.x_dpi, resolution.y_dpi))
        })
        .or_else(|| {
            supported
                .iter()
                .copied()
                .min_by_key(|resolution| (resolution.x_dpi, resolution.y_dpi))
        })
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Protocol,
                "scanner advertised no usable resolution",
            )
        })
}

fn resolve_format(requested: Option<&str>, supported: &[String]) -> Result<String> {
    if let Some(requested) = requested {
        if requested.is_empty() || !requested.contains('/') {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "a scan format must be a MIME media type",
            ));
        }
        return supported
            .iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(requested))
            .cloned()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Unsupported,
                    format!("scanner does not support requested format {requested}"),
                )
            });
    }

    ["image/jpeg", "application/pdf"]
        .into_iter()
        .find_map(|preferred| {
            supported
                .iter()
                .find(|candidate| candidate.eq_ignore_ascii_case(preferred))
                .cloned()
        })
        .or_else(|| supported.iter().min().cloned())
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Protocol,
                "scanner advertised no usable scan format",
            )
        })
}

/// Exact, validated settings for one scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanPlan {
    source: Source,
    sides: Sides,
    color_mode: ColorMode,
    resolution: Resolution,
    format: String,
    region: Region,
}

impl ScanPlan {
    /// Returns the selected source.
    #[must_use]
    pub const fn source(&self) -> Source {
        self.source
    }

    /// Returns the selected side mode.
    #[must_use]
    pub const fn sides(&self) -> Sides {
        self.sides
    }

    /// Returns the selected color mode.
    #[must_use]
    pub const fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    /// Returns the selected resolution.
    #[must_use]
    pub const fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Returns the selected MIME media type.
    #[must_use]
    pub fn format(&self) -> &str {
        &self.format
    }

    /// Returns the selected scan area.
    #[must_use]
    pub const fn region(&self) -> Region {
        self.region
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Capabilities, ColorMode, InputCapabilities, Region, Resolution, ScanOptions, Sides, Source,
    };
    use crate::Fingerprint;

    fn capabilities() -> Capabilities {
        Capabilities::new(vec![InputCapabilities {
            source: Source::Flatbed,
            sides: vec![Sides::One],
            color_modes: vec![ColorMode::Grayscale, ColorMode::Color],
            resolutions: vec![Resolution::dpi(150), Resolution::dpi(300)],
            formats: vec!["image/jpeg".to_owned(), "application/pdf".to_owned()],
            maximum_region: Region::millimeters(0, 0, 216, 297).expect("small dimensions"),
        }])
        .expect("valid capabilities")
    }

    #[test]
    fn defaults_resolve_deterministically() {
        let plan = capabilities()
            .resolve(&ScanOptions::new())
            .expect("defaults resolve");
        assert_eq!(plan.source(), Source::Flatbed);
        assert_eq!(plan.color_mode(), ColorMode::Color);
        assert_eq!(plan.resolution(), Resolution::dpi(300));
        assert_eq!(plan.format(), "image/jpeg");
    }

    #[test]
    fn explicit_values_are_not_rounded() {
        let error = capabilities()
            .resolve(&ScanOptions::new().dpi(200))
            .expect_err("200 dpi is unsupported");
        assert_eq!(error.kind(), crate::ErrorKind::Unsupported);
    }

    #[test]
    fn fingerprints_round_trip() {
        let text = "sha256:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
        let fingerprint = Fingerprint::parse(text).expect("valid fingerprint");
        assert_eq!(fingerprint.to_string(), text);
    }
}
