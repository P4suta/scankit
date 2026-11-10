// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A deliberately thin command-line client for the `scankit` public API.

use std::env;
use std::fs::File;
use std::io;
use std::process::ExitCode;
use std::time::Duration;

use scankit::{Capabilities, Scanner, Source};

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("skan: {error}");
            ExitCode::FAILURE
        },
    }
}

fn run(mut arguments: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    match arguments.next().as_deref() {
        Some("list") => {
            let seconds = arguments
                .next()
                .map(|value| value.parse::<u64>())
                .transpose()?
                .unwrap_or(3);
            for device in scankit::discover(Duration::from_secs(seconds))? {
                println!("{}\t{}", device.id(), device.name());
            }
            Ok(())
        },
        Some("info") => {
            let endpoint = required(arguments.next(), "info requires a scanner URL")?;
            let scanner = Scanner::connect(&endpoint)?;
            print_capabilities(scanner.capabilities());
            Ok(())
        },
        Some("scan") => {
            let endpoint = required(arguments.next(), "scan requires a scanner URL")?;
            let prefix = arguments.next().unwrap_or_else(|| "scan".to_owned());
            let mut scanner = Scanner::connect(&endpoint)?;
            let mut scan = scanner.scan()?;
            while let Some(mut page) = scan.next_page()? {
                let extension = page.suggested_extension().unwrap_or("bin");
                let path = format!("{prefix}-{:03}.{extension}", page.number());
                let mut file = File::create(&path)?;
                io::copy(&mut page, &mut file)?;
                println!("{path}");
            }
            Ok(())
        },
        Some("help" | "--help" | "-h") | None => {
            print_help();
            Ok(())
        },
        Some(command) => Err(format!("unknown command {command:?}; try `skan help`").into()),
    }
}

fn required(value: Option<String>, message: &str) -> Result<String, Box<dyn std::error::Error>> {
    value.ok_or_else(|| message.to_owned().into())
}

fn print_capabilities(capabilities: &Capabilities) {
    for source in capabilities.sources() {
        println!("{}", source_name(source));
        println!(
            "  sides: {}",
            capabilities
                .sides(source)
                .map(|value| format!("{value:?}"))
                .collect::<Vec<_>>()
                .join(", "),
        );
        println!(
            "  colors: {}",
            capabilities
                .color_modes(source)
                .map(|value| format!("{value:?}"))
                .collect::<Vec<_>>()
                .join(", "),
        );
        println!(
            "  resolutions: {}",
            capabilities
                .resolutions(source)
                .map(|value| format!("{}x{} dpi", value.x_dpi(), value.y_dpi()))
                .collect::<Vec<_>>()
                .join(", "),
        );
        println!(
            "  formats: {}",
            capabilities.formats(source).collect::<Vec<_>>().join(", "),
        );
    }
}

const fn source_name(source: Source) -> &'static str {
    match source {
        Source::Flatbed => "flatbed",
        Source::Feeder => "feeder",
        _ => "other",
    }
}

fn print_help() {
    println!(
        "skan - minimal driverless scanner client\n\n\
         Usage:\n\
           skan list [SECONDS]\n\
           skan info URL\n\
           skan scan URL [OUTPUT_PREFIX]"
    );
}
