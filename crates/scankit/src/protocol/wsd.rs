// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::{Error, ErrorKind, Intervention, Result};
use crate::model::{
    Capabilities, ColorMode, InputCapabilities, Region, Resolution, ScanPlan, Sides, Source,
};
use crate::protocol::xml::{Document, TextNode};
use crate::wire::{HttpCall, HttpReply};

const SOAP_CONTENT_TYPE: &str = "application/soap+xml; charset=utf-8";
const SCAN_NS: &str = "http://schemas.microsoft.com/windows/2006/08/wdp/scan";

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

pub(crate) fn capabilities_call(endpoint: &str) -> HttpCall {
    let body = soap_envelope(
        "http://schemas.microsoft.com/windows/2006/08/wdp/scan/GetScannerElements",
        "<scan:GetScannerElementsRequest><scan:RequestedElements>scan:ScannerDescription scan:ScannerConfiguration</scan:RequestedElements></scan:GetScannerElementsRequest>",
    );
    HttpCall::post(endpoint.to_owned(), SOAP_CONTENT_TYPE, body.into_bytes())
}

pub(crate) fn parse_capabilities(reply: &HttpReply) -> Result<Capabilities> {
    require_success(reply, "read WSD scanner elements")?;
    let document = Document::parse(&reply.body)?;
    if let Some(error) = soap_fault(&document) {
        return Err(error);
    }

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
        if let Some(target) = target {
            absorb_node(target, node)?;
        }
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

pub(crate) fn create_job_call(endpoint: &str, plan: &ScanPlan) -> Result<HttpCall> {
    let source = match plan.source() {
        Source::Flatbed => "Platen",
        Source::Feeder => "ADF",
    };
    let color = match plan.color_mode() {
        ColorMode::Monochrome => "BlackAndWhite1",
        ColorMode::Grayscale => "Grayscale8",
        ColorMode::Color => "RGB24",
    };
    let sides = match plan.sides() {
        Sides::One => "OneSided",
        Sides::Two => "TwoSidedLongEdge",
    };
    let resolution = plan.resolution();
    let region = plan.region();
    let body = format!(
        "<scan:CreateScanJobRequest><scan:ScanTicket><scan:JobDescription><scan:JobName>scankit</scan:JobName></scan:JobDescription><scan:DocumentParameters><scan:Format>{format}</scan:Format><scan:InputSource>{source}</scan:InputSource><scan:ContentType>TextAndGraphic</scan:ContentType><scan:InputSize><scan:InputMediaSize><scan:Width>{width}</scan:Width><scan:Height>{height}</scan:Height></scan:InputMediaSize></scan:InputSize><scan:Exposure><scan:ColorProcessing>{color}</scan:ColorProcessing></scan:Exposure><scan:Scaling><scan:ScalingWidth>{x_dpi}</scan:ScalingWidth><scan:ScalingHeight>{y_dpi}</scan:ScalingHeight></scan:Scaling><scan:Duplex>{sides}</scan:Duplex></scan:DocumentParameters></scan:ScanTicket></scan:CreateScanJobRequest>",
        format = escape_xml(plan.format()),
        width = micrometers_to_wsd_units(region.width_micrometers())?,
        height = micrometers_to_wsd_units(region.height_micrometers())?,
        x_dpi = resolution.x_dpi(),
        y_dpi = resolution.y_dpi(),
    );
    Ok(HttpCall::post(
        endpoint.to_owned(),
        SOAP_CONTENT_TYPE,
        soap_envelope(
            "http://schemas.microsoft.com/windows/2006/08/wdp/scan/CreateScanJob",
            &body,
        )
        .into_bytes(),
    ))
}

pub(crate) fn parse_job_id(reply: &HttpReply) -> Result<String> {
    require_success(reply, "create WSD scan job")?;
    let document = Document::parse(&reply.body)?;
    if let Some(error) = soap_fault(&document) {
        return Err(error);
    }
    document
        .first("JobId")
        .or_else(|| document.first("JobToken"))
        .map(str::to_owned)
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Protocol,
                "WSD scanner created a job without returning a job identifier",
            )
        })
}

pub(crate) fn retrieve_image_call(endpoint: &str, job_id: &str) -> HttpCall {
    let body = format!(
        "<scan:RetrieveImageRequest><scan:JobId>{}</scan:JobId><scan:DocumentDescription><scan:DocumentName>NextDocument</scan:DocumentName></scan:DocumentDescription></scan:RetrieveImageRequest>",
        escape_xml(job_id),
    );
    HttpCall::post(
        endpoint.to_owned(),
        SOAP_CONTENT_TYPE,
        soap_envelope(
            "http://schemas.microsoft.com/windows/2006/08/wdp/scan/RetrieveImage",
            &body,
        )
        .into_bytes(),
    )
}

pub(crate) fn cancel_call(endpoint: &str, job_id: &str) -> HttpCall {
    let body = format!(
        "<scan:CancelJobRequest><scan:JobId>{}</scan:JobId></scan:CancelJobRequest>",
        escape_xml(job_id),
    );
    HttpCall::post(
        endpoint.to_owned(),
        SOAP_CONTENT_TYPE,
        soap_envelope(
            "http://schemas.microsoft.com/windows/2006/08/wdp/scan/CancelJob",
            &body,
        )
        .into_bytes(),
    )
}

pub(crate) fn response_ends_job(reply: &HttpReply) -> bool {
    if matches!(reply.status, 404 | 409 | 410) {
        return true;
    }
    Document::parse(&reply.body).is_ok_and(|document| {
        document.contains_text("JobState", "Completed")
            || document.contains_text("JobState", "Canceled")
            || document.contains_text("JobStateReason", "JobCompletedSuccessfully")
    })
}

pub(crate) fn status_error(reply: &HttpReply) -> Option<Error> {
    let document = Document::parse(&reply.body).ok()?;
    soap_fault(&document)
}

fn absorb_node(input: &mut InputBuilder, node: &TextNode) -> Result<()> {
    let Some(name) = node.path.last().map(String::as_str) else {
        return Ok(());
    };
    match name {
        "ColorEntry" | "ColorProcessing" | "ColorMode" => {
            if let Some(mode) = parse_color_mode(&node.text) {
                push_unique(&mut input.color_modes, mode);
            }
        },
        "FormatEntry" | "Format" | "DocumentFormat" => {
            let format = node.text.trim().to_ascii_lowercase();
            if format.contains('/') && !input.formats.contains(&format) {
                input.formats.push(format);
            }
        },
        "XResolution" | "Resolution" | "ScalingWidth" => {
            let dpi = parse_u32(&node.text, "WSD resolution")?;
            if dpi != 0 {
                push_unique(&mut input.resolutions, Resolution::dpi(dpi));
            }
        },
        "MaxWidth" | "Width" => {
            let width = parse_u32(&node.text, "WSD maximum width")?;
            input.maximum_width = Some(wsd_units_to_micrometers(width)?);
        },
        "MaxHeight" | "Height" => {
            let height = parse_u32(&node.text, "WSD maximum height")?;
            input.maximum_height = Some(wsd_units_to_micrometers(height)?);
        },
        "Duplex" | "DuplexSupported"
            if node.text.eq_ignore_ascii_case("true")
                || node.text == "1"
                || node.text.to_ascii_lowercase().contains("two") =>
        {
            input.duplex = true;
        },
        _ => {},
    }
    if node.path.iter().any(|part| {
        part.to_ascii_lowercase().contains("duplex")
            || part.to_ascii_lowercase().contains("two-sided")
    }) {
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
            format!("WSD scanner omitted the maximum region for {source:?}"),
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
    if node
        .path
        .iter()
        .any(|part| part.to_ascii_lowercase().contains("platen"))
    {
        Some(Source::Flatbed)
    } else if node.path.iter().any(|part| {
        let lower = part.to_ascii_lowercase();
        lower.contains("adf") || lower.contains("feeder")
    }) {
        Some(Source::Feeder)
    } else {
        None
    }
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

fn soap_fault(document: &Document) -> Option<Error> {
    let reason = document
        .first("JobStateReason")
        .or_else(|| document.first("Text"))
        .or_else(|| document.first("FaultDetail"))?;
    let lower = reason.to_ascii_lowercase();
    if lower.contains("empty") {
        Some(Error::with_intervention(
            "automatic document feeder is empty",
            Intervention::EmptyFeeder,
        ))
    } else if lower.contains("jam") {
        Some(Error::with_intervention(
            "paper is jammed in the scanner",
            Intervention::PaperJam,
        ))
    } else if lower.contains("cover") || lower.contains("door") {
        Some(Error::with_intervention(
            "scanner cover is open",
            Intervention::CoverOpen,
        ))
    } else if lower.contains("busy") {
        Some(Error::with_intervention(
            "scanner is busy",
            Intervention::Busy,
        ))
    } else if lower.contains("warm") {
        Some(Error::with_intervention(
            "scanner is warming up",
            Intervention::WarmingUp,
        ))
    } else if lower.contains("complete") {
        None
    } else {
        Some(Error::new(
            ErrorKind::Protocol,
            format!("WSD scanner fault: {reason}"),
        ))
    }
}

fn soap_envelope(action: &str, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><s:Envelope xmlns:s=\"http://www.w3.org/2003/05/soap-envelope\" xmlns:wsa=\"http://schemas.xmlsoap.org/ws/2004/08/addressing\" xmlns:scan=\"{SCAN_NS}\"><s:Header><wsa:Action>{action}</wsa:Action></s:Header><s:Body>{body}</s:Body></s:Envelope>"
    )
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

fn wsd_units_to_micrometers(units: u32) -> Result<u32> {
    let scaled = u64::from(units)
        .checked_mul(25_400)
        .ok_or_else(|| Error::new(ErrorKind::Protocol, "WSD dimension overflowed"))?
        / 1_000;
    u32::try_from(scaled)
        .map_err(|error| Error::with_source(ErrorKind::Protocol, "WSD dimension overflowed", error))
}

fn micrometers_to_wsd_units(micrometers: u32) -> Result<u32> {
    let scaled = u64::from(micrometers)
        .checked_mul(1_000)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "scan region overflowed"))?
        / 25_400;
    u32::try_from(scaled).map_err(|error| {
        Error::with_source(ErrorKind::InvalidInput, "scan region overflowed", error)
    })
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
    use crate::{ColorMode, Resolution, Source};

    use super::parse_capabilities;

    #[test]
    fn scanner_elements_become_canonical_capabilities() {
        let body = br#"<scan:GetScannerElementsResponse xmlns:scan="http://schemas.microsoft.com/windows/2006/08/wdp/scan"><scan:ScannerConfiguration><scan:Platen><scan:PlatenConfiguration><scan:Width>8500</scan:Width><scan:Height>11700</scan:Height><scan:ColorEntry>RGB24</scan:ColorEntry><scan:FormatEntry>image/jpeg</scan:FormatEntry><scan:XResolution>300</scan:XResolution></scan:PlatenConfiguration></scan:Platen></scan:ScannerConfiguration></scan:GetScannerElementsResponse>"#;
        let capabilities = parse_capabilities(&HttpReply {
            status: 200,
            headers: Vec::new(),
            body: body.to_vec(),
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
}
