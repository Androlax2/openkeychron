//! openkeychron command-line tool.

mod keymap;
mod rgb;
mod session;
mod settings;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use openkeychron_hid::{HidTransport, KeychronDevice};
use openkeychron_keychron::{KNOWN_PROTOCOL_VERSION, Keychron, NotReported};
use openkeychron_via::Via;
use serde::Serialize;

use session::{Session, SessionError};

/// Configure Keychron keyboards without Keychron Launcher.
#[derive(Debug, Parser)]
#[command(name = "openkeychron", version)]
struct Cli {
    /// Replay a recorded session instead of talking to hardware.
    #[arg(long, global = true, value_name = "FILE", conflicts_with = "record")]
    fixture: Option<PathBuf>,

    /// Record the session with the device to FILE (a fixture, see docs/FIXTURES.md).
    #[arg(long, global = true, value_name = "FILE")]
    record: Option<PathBuf>,

    /// Talk to the device at PATH (from `openkeychron list`) when several are connected.
    #[arg(long, global = true, value_name = "PATH", conflicts_with = "fixture")]
    device: Option<PathBuf>,

    /// Print JSON instead of text.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List connected Keychron devices that have a VIA interface.
    List,
    /// Print the VIA protocol and firmware values of the one connected device. Only reads.
    Info {
        /// Wait up to this many milliseconds per reply (1–60000; default: 500).
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..=60_000))]
        reply_timeout_ms: Option<u64>,
    },
    /// Display keymap layers for the V5 Max ISO, with French position labels. Only reads.
    Keymap {
        #[command(subcommand)]
        action: keymap::KeymapAction,
    },
    /// Read or change the RGB effect, speed, brightness and colour. Writes only over a USB cable.
    Rgb {
        #[command(subcommand)]
        action: rgb::RgbAction,
    },
    /// Read or change debounce and Snap Click. Writes only over a USB cable.
    Settings {
        #[command(subcommand)]
        action: settings::SettingsAction,
    },
}

#[derive(Debug, Serialize)]
struct DeviceOutput {
    name: Option<String>,
    vendor_id: u16,
    product_id: u16,
    path: String,
    is_receiver: bool,
}

#[derive(Debug, Serialize)]
struct InfoOutput {
    device: DeviceOutput,
    via_protocol_version: u16,
    via_firmware_version: u32,
    layout_options: u32,
    uptime_ms: u64,
    /// Keychron's own version (`0xA1`), `null` when not reported.
    keychron_firmware_version: Option<String>,
    keychron_firmware_build_date: Option<String>,
    /// Why the Keychron version is missing, `null` when it is reported.
    keychron_firmware_not_reported: Option<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("openkeychron: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), SessionError> {
    match &cli.command {
        Command::List => list(cli),
        Command::Info { reply_timeout_ms } => info(cli, *reply_timeout_ms),
        Command::Keymap { action } => keymap::run(
            action,
            cli.fixture.as_deref(),
            cli.record.as_deref(),
            cli.device.as_deref(),
            cli.json,
        ),
        Command::Rgb { action } => rgb::run(
            action,
            cli.fixture.as_deref(),
            cli.record.as_deref(),
            cli.device.as_deref(),
            cli.json,
        ),
        Command::Settings { action } => settings::run(
            action,
            cli.fixture.as_deref(),
            cli.record.as_deref(),
            cli.device.as_deref(),
            cli.json,
        ),
    }
}

fn list(cli: &Cli) -> Result<(), SessionError> {
    if cli.device.is_some() {
        return Err(SessionError::Usage(
            "--device selects a device for commands such as `info`; `list` shows all devices"
                .to_owned(),
        ));
    }
    if cli.record.is_some() {
        return Err(SessionError::Usage(
            "--record applies to commands that talk to a device, such as `info`".to_owned(),
        ));
    }
    let devices = session::list(cli.fixture.as_deref())?;
    if cli.json {
        let output: Vec<DeviceOutput> = devices.iter().map(device_output).collect();
        print_json(&output)?;
    } else if devices.is_empty() {
        println!("No Keychron device with a VIA interface found.");
    } else {
        for device in &devices {
            println!("{}", describe(device));
        }
    }
    Ok(())
}

fn info(cli: &Cli, reply_timeout_ms: Option<u64>) -> Result<(), SessionError> {
    let session = Session::start(
        cli.fixture.as_deref(),
        cli.record.as_deref(),
        cli.device.as_deref(),
    )?
    .with_reply_timeout(reply_timeout_ms.map(Duration::from_millis));
    let device = session.device().clone();
    let output = query_info(session)?;

    if cli.json {
        print_json(&output)?;
    } else {
        println!("Device:               {}", describe(&device));
        println!(
            "VIA protocol:         0x{:04x}",
            output.via_protocol_version
        );
        println!(
            "VIA firmware version: 0x{:08x}",
            output.via_firmware_version
        );
        println!("Layout options:       0x{:08x}", output.layout_options);
        println!(
            "Uptime:               {:.3} s",
            Duration::from_millis(output.uptime_ms).as_secs_f64()
        );
        match (
            &output.keychron_firmware_version,
            &output.keychron_firmware_build_date,
            &output.keychron_firmware_not_reported,
        ) {
            (Some(version), Some(date), _) => {
                println!("Keychron firmware:    {version} (built {date})");
            }
            (_, _, Some(reason)) => println!("Keychron firmware:    not reported ({reason})"),
            _ => println!("Keychron firmware:    not reported"),
        }
    }
    Ok(())
}

fn query_info<T: HidTransport>(session: Session<T>) -> Result<InfoOutput, SessionError> {
    let device = session.device().clone();
    let mut via = Via::new(session);
    let result =
        read_info(&mut via, &device).map_err(|error| SessionError::via(&error, device.is_receiver));
    let mut keychron = Keychron::new(via.into_transport());
    let result = result.and_then(|mut output| {
        match keychron.firmware_version()? {
            Ok(firmware) => {
                output.keychron_firmware_version = Some(firmware.version);
                output.keychron_firmware_build_date = Some(firmware.build_date);
            }
            Err(reason) => output.keychron_firmware_not_reported = Some(not_reported(reason)),
        }
        Ok(output)
    });
    keychron.into_transport().complete(result)
}

fn not_reported(reason: NotReported) -> String {
    match reason {
        NotReported::NoKeychronProtocol => {
            "the firmware does not answer Keychron command 0xa0".to_owned()
        }
        NotReported::UnknownProtocolVersion(version) => format!(
            "Keychron protocol version {version}; this build reads version {KNOWN_PROTOCOL_VERSION}"
        ),
    }
}

fn read_info<T: HidTransport>(
    via: &mut Via<T>,
    device: &KeychronDevice,
) -> Result<InfoOutput, openkeychron_via::ViaError> {
    Ok(InfoOutput {
        device: device_output(device),
        via_protocol_version: via.protocol_version()?,
        via_firmware_version: via.via_firmware_version()?,
        layout_options: via.layout_options()?,
        uptime_ms: u64::try_from(via.uptime()?.as_millis()).unwrap_or(u64::MAX),
        keychron_firmware_version: None,
        keychron_firmware_build_date: None,
        keychron_firmware_not_reported: None,
    })
}

fn device_output(device: &KeychronDevice) -> DeviceOutput {
    DeviceOutput {
        name: device.name.clone(),
        vendor_id: device.vendor_id,
        product_id: device.product_id,
        path: device.path.clone(),
        is_receiver: device.is_receiver,
    }
}

fn describe(device: &KeychronDevice) -> String {
    let name = device.name.as_deref().unwrap_or("(no product name)");
    let receiver = if device.is_receiver { ", receiver" } else { "" };
    format!(
        "{name} ({:04x}:{:04x}{receiver}) at {}",
        device.vendor_id, device.product_id, device.path
    )
}

pub(crate) fn print_json(value: &impl Serialize) -> Result<(), SessionError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| SessionError::Usage(format!("cannot encode JSON: {error}")))?;
    println!("{text}");
    Ok(())
}
