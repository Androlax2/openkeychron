//! Finding Keychron devices and opening their VIA raw HID interface through `hidapi`.

use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use thiserror::Error;

use crate::{HidTransport, TransportError};

pub const KEYCHRON_VENDOR_ID: u16 = 0x3434;

/// The raw HID interface VIA uses: usage page `0xFF60`, usage `0x61` (QMK `RAW_USAGE_PAGE`,
/// `RAW_USAGE_ID`).
pub const VIA_USAGE_PAGE: u16 = 0xff60;
pub const VIA_USAGE: u16 = 0x61;

/// USB receivers whose VIA interface may lead to a wireless keyboard rather than being one.
/// Only receivers seen on hardware are listed (`docs/protocol/keychron-link.md`).
const RECEIVER_PRODUCT_IDS: &[u16] = &[0xd030];

/// Where the fix for a permission error is documented.
const UDEV_RULE_HINT: &str =
    "install the udev rule packaging/linux/70-openkeychron.rules (see docs/INSTALL-linux.md)";

/// A Keychron device's VIA interface, found but not opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeychronDevice {
    /// The USB product string, when the device reports one.
    pub name: Option<String>,
    pub vendor_id: u16,
    pub product_id: u16,
    /// The OS path of the interface, `/dev/hidrawN` on Linux.
    pub path: String,
    /// True for a USB receiver such as the Keychron Link: the keyboard behind it is wireless.
    pub is_receiver: bool,
}

/// One HID interface as the OS lists it: the input to [`keychron_via_interfaces`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidInterface {
    pub name: Option<String>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub path: String,
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("cannot list HID devices: {0}")]
    Enumerate(String),
}

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("permission denied opening {path}: {UDEV_RULE_HINT}")]
    PermissionDenied { path: String },

    #[error("{path} no longer exists: was the device unplugged?")]
    NotFound { path: String },

    #[error("cannot open {path}: {message}")]
    Other { path: String, message: String },
}

/// Keeps the Keychron interfaces that speak VIA, and nothing else.
pub fn keychron_via_interfaces(
    interfaces: impl IntoIterator<Item = HidInterface>,
) -> Vec<KeychronDevice> {
    interfaces
        .into_iter()
        .filter(|interface| {
            interface.vendor_id == KEYCHRON_VENDOR_ID
                && interface.usage_page == VIA_USAGE_PAGE
                && interface.usage == VIA_USAGE
        })
        .map(|interface| KeychronDevice {
            is_receiver: RECEIVER_PRODUCT_IDS.contains(&interface.product_id),
            name: interface.name,
            vendor_id: interface.vendor_id,
            product_id: interface.product_id,
            path: interface.path,
        })
        .collect()
}

/// Lists the VIA interfaces of connected Keychron devices. Reads the OS device list only;
/// no device is opened.
pub fn list_keychron_devices() -> Result<Vec<KeychronDevice>, DiscoveryError> {
    let api =
        hidapi::HidApi::new().map_err(|error| DiscoveryError::Enumerate(error.to_string()))?;
    let interfaces = api.device_list().map(|info| HidInterface {
        name: info.product_string().map(str::to_owned),
        vendor_id: info.vendor_id(),
        product_id: info.product_id(),
        usage_page: info.usage_page(),
        usage: info.usage(),
        path: info.path().to_string_lossy().into_owned(),
    });
    Ok(keychron_via_interfaces(interfaces))
}

/// Opens a device's VIA interface.
///
/// The node is first opened with the standard library, because `hidapi`'s Linux backend turns
/// a permission error into plain text and the user needs to be told about the udev rule.
pub fn open(device: &KeychronDevice) -> Result<HidapiTransport, OpenError> {
    let path = device.path.clone();
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(Path::new(&path))
        .map_err(|error| match error.kind() {
            ErrorKind::PermissionDenied => OpenError::PermissionDenied { path: path.clone() },
            ErrorKind::NotFound => OpenError::NotFound { path: path.clone() },
            _ => OpenError::Other {
                path: path.clone(),
                message: error.to_string(),
            },
        })?;

    let other = |message: String| OpenError::Other {
        path: path.clone(),
        message,
    };
    let api = hidapi::HidApi::new().map_err(|error| other(error.to_string()))?;
    let c_path = std::ffi::CString::new(path.clone())
        .map_err(|_| other("the path contains a NUL byte".to_owned()))?;
    let device = api
        .open_path(&c_path)
        .map_err(|error| other(error.to_string()))?;
    Ok(HidapiTransport { device })
}

/// [`HidTransport`] over a real device, through `hidapi`.
pub struct HidapiTransport {
    device: hidapi::HidDevice,
}

impl std::fmt::Debug for HidapiTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HidapiTransport")
            .finish_non_exhaustive()
    }
}

/// The largest input report read back. VIA reports are 32 bytes; the margin keeps a longer
/// report visible as a wrong length instead of silently cut.
const READ_BUFFER_LEN: usize = 64;

impl HidTransport for HidapiTransport {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        let framed = with_report_id(report);
        let written = self
            .device
            .write(&framed)
            .map_err(|error| TransportError::Device(error.to_string()))?;
        if written != framed.len() {
            return Err(TransportError::Device(format!(
                "wrote {written} of {} bytes",
                framed.len()
            )));
        }
        Ok(())
    }

    fn read(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError> {
        let mut buffer = [0u8; READ_BUFFER_LEN];
        let milliseconds = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
        let read = self
            .device
            .read_timeout(&mut buffer, milliseconds)
            .map_err(|error| TransportError::Device(error.to_string()))?;
        if read == 0 {
            return Err(TransportError::Timeout);
        }
        Ok(buffer[..read].to_vec())
    }
}

/// The VIA interface has no numbered reports, so the OS expects report ID 0 before the data.
fn with_report_id(report: &[u8]) -> Vec<u8> {
    let mut framed = Vec::with_capacity(report.len() + 1);
    framed.push(0x00);
    framed.extend_from_slice(report);
    framed
}

#[cfg(test)]
mod tests {
    use super::{HidInterface, KeychronDevice, keychron_via_interfaces, with_report_id};

    fn interface(vendor_id: u16, product_id: u16, usage_page: u16, usage: u16) -> HidInterface {
        HidInterface {
            name: Some("Keychron Link".to_owned()),
            vendor_id,
            product_id,
            usage_page,
            usage,
            path: format!("/dev/hidraw-{product_id:04x}-{usage_page:04x}"),
        }
    }

    #[test]
    fn keeps_the_via_interface_of_a_keychron_device() {
        let found = keychron_via_interfaces([interface(0x3434, 0x0951, 0xff60, 0x61)]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].product_id, 0x0951);
    }

    #[test]
    fn drops_other_interfaces_of_the_same_device() {
        let found = keychron_via_interfaces([
            interface(0x3434, 0xd030, 0x0001, 0x06),
            interface(0x3434, 0xd030, 0x0001, 0x02),
            interface(0x3434, 0xd030, 0x008c, 0x01),
        ]);
        assert_eq!(found, Vec::<KeychronDevice>::new());
    }

    #[test]
    fn drops_a_via_interface_from_another_vendor() {
        let found = keychron_via_interfaces([interface(0x046d, 0xc548, 0xff60, 0x61)]);
        assert_eq!(found, Vec::<KeychronDevice>::new());
    }

    #[test]
    fn drops_the_via_usage_page_with_another_usage() {
        let found = keychron_via_interfaces([interface(0x3434, 0x0951, 0xff60, 0x62)]);
        assert_eq!(found, Vec::<KeychronDevice>::new());
    }

    #[test]
    fn marks_the_keychron_link_as_a_receiver() {
        let found = keychron_via_interfaces([interface(0x3434, 0xd030, 0xff60, 0x61)]);
        assert!(found[0].is_receiver);
    }

    #[test]
    fn does_not_mark_a_cable_connected_keyboard_as_a_receiver() {
        let found = keychron_via_interfaces([interface(0x3434, 0x0951, 0xff60, 0x61)]);
        assert!(!found[0].is_receiver);
    }

    #[test]
    fn keeps_name_ids_and_path() {
        let found = keychron_via_interfaces([interface(0x3434, 0xd030, 0xff60, 0x61)]);
        assert_eq!(
            found[0],
            KeychronDevice {
                name: Some("Keychron Link".to_owned()),
                vendor_id: 0x3434,
                product_id: 0xd030,
                path: "/dev/hidraw-d030-ff60".to_owned(),
                is_receiver: true,
            }
        );
    }

    #[test]
    fn prefixes_report_id_zero() {
        assert_eq!(with_report_id(&[0x01, 0x02]), vec![0x00, 0x01, 0x02]);
    }
}
