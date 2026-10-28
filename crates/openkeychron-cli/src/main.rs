//! openkeychron command-line tool.

mod session;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use openkeychron_hid::KeychronDevice;
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
    Info,
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
    match cli.command {
        Command::List => list(cli),
        Command::Info => info(cli),
    }
}

fn list(cli: &Cli) -> Result<(), SessionError> {
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

fn info(cli: &Cli) -> Result<(), SessionError> {
    let session = Session::start(cli.fixture.as_deref(), cli.record.as_deref())?;
    let device = session.device().clone();
    let mut via = Via::new(session);
    let output = read_info(&mut via, &device)
        .map_err(|error| SessionError::via(&error, device.is_receiver))?;
    via.into_transport().finish()?;

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
    }
    Ok(())
}

fn read_info(
    via: &mut Via<Session>,
    device: &KeychronDevice,
) -> Result<InfoOutput, openkeychron_via::ViaError> {
    Ok(InfoOutput {
        device: device_output(device),
        via_protocol_version: via.protocol_version()?,
        via_firmware_version: via.via_firmware_version()?,
        layout_options: via.layout_options()?,
        uptime_ms: u64::try_from(via.uptime()?.as_millis()).unwrap_or(u64::MAX),
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

fn print_json(value: &impl Serialize) -> Result<(), SessionError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| SessionError::Usage(format!("cannot encode JSON: {error}")))?;
    println!("{text}");
    Ok(())
}
