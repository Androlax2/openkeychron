//! `list_devices`, and finding the device a command names by its path.

use openkeychron_hid::KeychronDevice;
use serde::Serialize;

use crate::error::CommandError;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// The USB product string, when the device reports one.
    pub name: Option<String>,
    pub vendor_id: u16,
    pub product_id: u16,
    /// What the other commands take to name this device.
    pub path: String,
    /// True for a USB receiver such as the Keychron Link: the keyboard behind it is wireless.
    pub is_receiver: bool,
}

impl From<KeychronDevice> for Device {
    fn from(device: KeychronDevice) -> Self {
        Self {
            name: device.name,
            vendor_id: device.vendor_id,
            product_id: device.product_id,
            path: device.path,
            is_receiver: device.is_receiver,
        }
    }
}

/// The listed device at `path`. A path that is not listed is refused rather than opened.
pub fn find(mut devices: Vec<KeychronDevice>, path: &str) -> Result<KeychronDevice, CommandError> {
    match devices.iter().position(|device| device.path == path) {
        Some(index) => Ok(devices.swap_remove(index)),
        None => Err(not_connected(&devices, path)),
    }
}

/// The refusal for a `path` that is none of `devices`, naming the ones that are listed.
pub fn not_connected(devices: &[KeychronDevice], path: &str) -> CommandError {
    let connected = if devices.is_empty() {
        "none".to_owned()
    } else {
        devices
            .iter()
            .map(|device| device.path.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    CommandError::NotConnected {
        path: path.to_owned(),
        connected,
    }
}

#[cfg(test)]
mod tests {
    use openkeychron_hid::KeychronDevice;

    use super::{Device, find};

    fn device(path: &str, product_id: u16) -> KeychronDevice {
        KeychronDevice {
            name: Some("test".to_owned()),
            vendor_id: 0x3434,
            product_id,
            path: path.to_owned(),
            is_receiver: product_id == 0xd030,
        }
    }

    fn receiver_and_keyboard() -> Vec<KeychronDevice> {
        vec![
            device("/dev/hidraw12", 0xd030),
            device("/dev/hidraw20", 0x0951),
        ]
    }

    #[test]
    fn finds_the_listed_device_at_that_path() {
        let found = find(receiver_and_keyboard(), "/dev/hidraw20").unwrap();
        assert_eq!(found, device("/dev/hidraw20", 0x0951));
    }

    #[test]
    fn refuses_a_path_that_is_not_listed_and_names_the_listed_ones() {
        let error = find(receiver_and_keyboard(), "/dev/hidraw19").unwrap_err();
        assert_eq!(
            error.to_string(),
            "/dev/hidraw19 is not a connected Keychron VIA interface; connected: /dev/hidraw12, \
             /dev/hidraw20"
        );
    }

    #[test]
    fn refuses_any_path_when_nothing_is_connected() {
        let error = find(vec![], "/dev/hidraw20").unwrap_err();
        assert_eq!(
            error.to_string(),
            "/dev/hidraw20 is not a connected Keychron VIA interface; connected: none"
        );
    }

    #[test]
    fn a_device_reaches_the_ui_with_camel_case_fields() {
        let json = serde_json::to_value(Device::from(device("/dev/hidraw12", 0xd030))).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "name": "test",
                "vendorId": 0x3434,
                "productId": 0xd030,
                "path": "/dev/hidraw12",
                "isReceiver": true,
            })
        );
    }
}
