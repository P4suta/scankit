// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::{Error, ErrorKind, Intervention, Result};
use crate::model::{
    Capabilities, ColorMode, InputCapabilities, Region, Resolution, ScanPlan, Sides, Source,
};
use crate::protocol::xml::{Document, TextNode};
use crate::wire::{HttpCall, HttpReply};

const ESCL_CONTENT_TYPE: &str = "text/xml; charset=utf-8";

#[derive(Debug, Default)]
struct InputBuilder {
    source: Option<Source>,
    duplex: bool,
    color_modes: Vec<ColorMode>,
    resolutions: Vec<Resolution>,
    formats: Vec<String>,
    maximum_width: Option<u32>,
    maximum_height: Option<u32>,
}

pub(crate) fn capabilities_call(root: &str) -> HttpCall {
    HttpCall::get(join(root, "ScannerCapabilities"))
}

pub(crate) fn parse_capabilities(reply: &HttpReply) -> Result<Capabilities> {
    require_success(reply, "read eSCL capabilities")?;
    let document = Document::parse(&reply.body)?;
    let mut platen = InputBuilder {
        source: Some(Source::Flatbed),
        ..InputBuilder::default()
    };
    let mut feeder = InputBuilder {
        source: Some(Source::Feeder),
        ..InputBuilder::default()
    };

    for node in document.nodes() {
        let target = source_for(node).map(|source| match source {
            Source::Flatbed => &mut platen,
            Source::Feeder => &mut feeder,
        });
        let Some(target) = target else {
            continue;
        };
        absorb_node(target, node)?;
    }

    let mut inputs = Vec::new();
    if let Some(input) = finish_input(platen)? {
        inputs.push(input);
    }
    if let Some(input) = finish_input(feeder)? {
        inputs.push(input);
    }
    Capabilities::new(inputs)
}

pub(crate) fn create_job_call(root: &str, plan: &ScanPlan) -> Result<HttpCall> {
    let xml = render_scan_settings(plan)?;
    Ok(HttpCall::post(
        join(root, "ScanJobs"),
        ESCL_CONTENT_TYPE,
        xml.into_bytes(),
    ))
}

pub(crate) fn parse_job_location(reply: &HttpReply) -> Result<String> {
    require_success(reply, "create eSCL scan job")?;
    reply.header("location").map(str::to_owned).ok_or_else(|| {
        Error::new(
            ErrorKind::Protocol,
            "eSCL scanner created a job without returning its location",
        )
    })
}

pub(crate) fn next_document_call(job_url: &str) -> HttpCall {
    HttpCall::get(join(job_url, "NextDocument"))
}

pub(crate) fn cancel_call(job_url: &str) -> HttpCall {
    HttpCall::delete(job_url.to_owned())
}

pub(crate) fn response_ends_job(status: u16) -> bool {
    matches!(status, 404 | 409 | 410)
}

pub(crate) fn status_error(reply: &HttpReply) -> Option<Error> {
    let document = Document::parse(&reply.body).ok()?;
    if document.contains_text("AdfState", "ScannerAdfEmpty")
        || document.contains_text("JobStateReason", "JobInputSourceEmpty")
    {
        return Some(Error::with_intervention(
            "automatic document feeder is empty",
            Intervention::EmptyFeeder,
        ));
    }
    if document.contains_text("AdfState", "ScannerAdfJam")
        || document.contains_text("JobStateReason", "JobMediaJam")
    {
        return Some(Error::with_intervention(
            "paper is jammed in the scanner",
            Intervention::PaperJam,
        ));
    }
    if document.contains_text("PlatenState", "ScannerPlatenOpen")
        || document.contains_text("JobStateReason", "JobCoverOpen")
    {
        return Some(Error::with_intervention(
            "scanner cover is open",
            Intervention::CoverOpen,
        ));
    }
    if document.contains_text("ScannerState", "Processing")
        || document.contains_text("JobStateReason", "JobScanning")
    {
        return Some(Error::new(ErrorKind::Unavailable, "scanner is still processing").retryable());
    }
    None
}

fn absorb_node(input: &mut InputBuilder, node: &TextNode) -> Result<()> {
    let Some(name) = node.path.last().map(String::as_str) else {
        return Ok(());
    };
    match name {
        "ColorMode" => {
            if let Some(mode) = parse_color_mode(&node.text) {
                push_unique(&mut input.color_modes, mode);
            }
        },
        "DocumentFormat" | "DocumentFormatExt" => {
            let format = node.text.trim().to_ascii_lowercase();
            if format.contains('/') && !input.formats.contains(&format) {
                input.formats.push(format);
            }
        },
        "XResolution" => {
            let dpi = parse_u32(&node.text, "eSCL resolution")?;
            if dpi != 0 {
                push_unique(&mut input.resolutions, Resolution::dpi(dpi));
            }
        },
        "MaxWidth" => {
            input.maximum_width = Some(escl_units_to_micrometers(parse_u32(
                &node.text,
                "eSCL maximum width",
            )?)?);
        },
        "MaxHeight" => {
            input.maximum_height = Some(escl_units_to_micrometers(parse_u32(
                &node.text,
                "eSCL maximum height",
            )?)?);
        },
        "Duplex" if node.text.eq_ignore_ascii_case("true") || node.text == "1" => {
            input.duplex = true;
        },
        _ => {},
    }
    if node
        .path
        .iter()
        .any(|part| part.eq_ignore_ascii_case("AdfDuplexInputCaps"))
    {
        input.duplex = true;
    }
    Ok(())
}

fn finish_input(mut builder: InputBuilder) -> Result<Option<InputCapabilities>> {
    let Some(source) = builder.source else {
        return Ok(None);
    };
    if builder.color_modes.is_empty()
        && builder.resolutions.is_empty()
        && builder.formats.is_empty()
    {
        return Ok(None);
    }
    let (Some(width), Some(height)) = (builder.maximum_width, builder.maximum_height) else {
        return Err(Error::new(
            ErrorKind::Protocol,
            format!("eSCL scanner omitted the maximum region for {source:?}"),
        ));
    };

    builder
        .resolutions
        .sort_by_key(|value| (value.x_dpi(), value.y_dpi()));
    builder.formats.sort();
    let mut sides = vec![Sides::One];
    if builder.duplex {
        sides.push(Sides::Two);
    }
    Ok(Some(InputCapabilities {
        source,
        sides,
        color_modes: builder.color_modes,
        resolutions: builder.resolutions,
        formats: builder.formats,
        maximum_region: Region::micrometers(0, 0, width, height),
    }))
}

fn source_for(node: &TextNode) -> Option<Source> {
    if node.path.iter().any(|part| {
        part.eq_ignore_ascii_case("Platen") || part.eq_ignore_ascii_case("PlatenInputCaps")
    }) {
        return Some(Source::Flatbed);
    }
    if node
        .path
        .iter()
        .any(|part| part.to_ascii_lowercase().contains("adf"))
    {
        return Some(Source::Feeder);
    }
    None
}

fn parse_color_mode(value: &str) -> Option<ColorMode> {
    let normalized = value.to_ascii_lowercase();
    if normalized.contains("rgb") || normalized.contains("color") {
        Some(ColorMode::Color)
    } else if normalized.contains("gray") {
        Some(ColorMode::Grayscale)
    } else if normalized.contains("black")
        || normalized.contains("white")
        || normalized.contains("mono")
    {
        Some(ColorMode::Monochrome)
    } else {
        None
    }
}

fn parse_u32(value: &str, field: &str) -> Result<u32> {
    value.parse::<u32>().map_err(|error| {
        Error::with_source(
            ErrorKind::Protocol,
            format!("scanner returned an invalid {field}"),
            error,
        )
    })
}

fn escl_units_to_micrometers(units: u32) -> Result<u32> {
    let scaled = u64::from(units)
        .checked_mul(25_400)
        .ok_or_else(|| Error::new(ErrorKind::Protocol, "eSCL dimension overflowed"))?
        / 300;
    u32::try_from(scaled).map_err(|error| {
        Error::with_source(ErrorKind::Protocol, "eSCL dimension overflowed", error)
    })
}

fn micrometers_to_escl_units(micrometers: u32) -> Result<u32> {
    let scaled = u64::from(micrometers)
        .checked_mul(300)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "scan region overflowed"))?
        / 25_400;
    u32::try_from(scaled).map_err(|error| {
        Error::with_source(ErrorKind::InvalidInput, "scan region overflowed", error)
    })
}

fn render_scan_settings(plan: &ScanPlan) -> Result<String> {
    let source = match plan.source() {
        Source::Flatbed => "Platen",
        Source::Feeder => "Feeder",
    };
    let color_mode = match plan.color_mode() {
        ColorMode::Monochrome => "BlackAndWhite1",
        ColorMode::Grayscale => "Grayscale8",
        ColorMode::Color => "RGB24",
    };
    let duplex = match plan.sides() {
        Sides::One => "false",
        Sides::Two => "true",
    };
    let resolution = plan.resolution();
    let region = plan.region();
    let x_offset = micrometers_to_escl_units(region.x_micrometers())?;
    let y_offset = micrometers_to_escl_units(region.y_micrometers())?;
    let width = micrometers_to_escl_units(region.width_micrometers())?;
    let height = micrometers_to_escl_units(region.height_micrometers())?;
    let format = escape_xml(plan.format());

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<scan:ScanSettings xmlns:scan=\"http://schemas.hp.com/imaging/escl/2011/05/03\" \
xmlns:pwg=\"http://www.pwg.org/schemas/2010/12/sm\">\
<pwg:Version>2.0</pwg:Version>\
<scan:Intent>Document</scan:Intent>\
<pwg:ScanRegions><pwg:ScanRegion>\
<pwg:Height>{height}</pwg:Height><pwg:Width>{width}</pwg:Width>\
<pwg:XOffset>{x_offset}</pwg:XOffset><pwg:YOffset>{y_offset}</pwg:YOffset>\
</pwg:ScanRegion></pwg:ScanRegions>\
<pwg:InputSource>{source}</pwg:InputSource>\
<scan:Duplex>{duplex}</scan:Duplex>\
<scan:ColorMode>{color_mode}</scan:ColorMode>\
<scan:XResolution>{x_dpi}</scan:XResolution>\
<scan:YResolution>{y_dpi}</scan:YResolution>\
<pwg:DocumentFormat>{format}</pwg:DocumentFormat>\
</scan:ScanSettings>",
        x_dpi = resolution.x_dpi(),
        y_dpi = resolution.y_dpi(),
    ))
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn join(base: &str, suffix: &str) -> String {
    format!(
        "{base}/{suffix}",
        base = base.trim_end_matches('/'),
        suffix = suffix.trim_start_matches('/'),
    )
}

fn require_success(reply: &HttpReply, operation: &str) -> Result<()> {
    if (200..300).contains(&reply.status) {
        Ok(())
    } else if let Some(error) = status_error(reply) {
        Err(error)
    } else {
        Err(Error::new(
            ErrorKind::Protocol,
            format!("could not {operation}: HTTP {}", reply.status),
        ))
    }
}

fn push_unique<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use crate::wire::HttpReply;
    use crate::{ColorMode, Resolution, ScanOptions, Sides, Source};

    use super::{parse_capabilities, render_scan_settings};

    const CAPABILITIES: &str = r#"
      <scan:ScannerCapabilities xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03"
          xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm">
        <scan:Platen><scan:PlatenInputCaps>
          <scan:MaxWidth>2550</scan:MaxWidth><scan:MaxHeight>3507</scan:MaxHeight>
          <scan:ColorModes><scan:ColorMode>RGB24</scan:ColorMode></scan:ColorModes>
          <scan:DocumentFormats><pwg:DocumentFormat>image/jpeg</pwg:DocumentFormat></scan:DocumentFormats>
          <scan:SupportedResolutions><scan:DiscreteResolutions>
            <scan:DiscreteResolution><scan:XResolution>300</scan:XResolution><scan:YResolution>300</scan:YResolution></scan:DiscreteResolution>
          </scan:DiscreteResolutions></scan:SupportedResolutions>
        </scan:PlatenInputCaps></scan:Platen>
      </scan:ScannerCapabilities>
    "#;

    #[test]
    fn capability_xml_becomes_one_canonical_model() {
        let capabilities = parse_capabilities(&HttpReply {
            status: 200,
            headers: Vec::new(),
            body: CAPABILITIES.as_bytes().to_vec(),
        })
        .expect("valid capabilities");
        assert_eq!(
            capabilities.sources().collect::<Vec<_>>(),
            [Source::Flatbed]
        );
        assert_eq!(
            capabilities
                .color_modes(Source::Flatbed)
                .collect::<Vec<_>>(),
            [ColorMode::Color],
        );
        assert_eq!(
            capabilities
                .resolutions(Source::Flatbed)
                .collect::<Vec<_>>(),
            [Resolution::dpi(300)],
        );
    }

    #[test]
    fn scan_settings_are_rendered_from_a_resolved_plan() {
        let capabilities = parse_capabilities(&HttpReply {
            status: 200,
            headers: Vec::new(),
            body: CAPABILITIES.as_bytes().to_vec(),
        })
        .expect("valid capabilities");
        let plan = capabilities
            .resolve(&ScanOptions::new().source(Source::Flatbed).sides(Sides::One))
            .expect("supported plan");
        let xml = render_scan_settings(&plan).expect("renderable");
        assert!(xml.contains("<pwg:InputSource>Platen</pwg:InputSource>"));
        assert!(xml.contains("<scan:XResolution>300</scan:XResolution>"));
    }
}
