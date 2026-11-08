//! Which device may be written to: a model the built-in registry marks as verified for writes,
//! on a USB cable.

use openkeychron_hid::KeychronDevice;
use openkeychron_via::{DeviceRegistry, RegisteredModel, WriteAccess};

use crate::GuardError;

/// Proof that [`require_write_verified`] accepted the device. Every guarded write asks for it,
/// so none can run on a device the gate did not check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteVerified {
    device: KeychronDevice,
}

impl WriteVerified {
    #[must_use]
    pub fn device(&self) -> &KeychronDevice {
        &self.device
    }
}

/// The model behind `vendor_id:product_id` when the built-in registry marks it as verified for
/// writes. Only the built-in registry is asked, so a layout file loaded at runtime can never
/// enable a write.
#[must_use]
pub fn write_verified_model(vendor_id: u16, product_id: u16) -> Option<RegisteredModel> {
    DeviceRegistry::built_in()
        .model(vendor_id, product_id)
        .filter(|model| model.write_access() == WriteAccess::VerifiedOverCable)
        .cloned()
}

/// The verified models as `the V5 Max ISO (3434:0951)`, for refusal messages.
#[must_use]
pub fn write_verified_models_text() -> String {
    DeviceRegistry::built_in()
        .write_verified_models()
        .map(|((vendor_id, product_id), model)| {
            format!("the {} ({vendor_id:04x}:{product_id:04x})", model.name())
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

/// Refuses `device` unless it is on a USB cable and its model is verified for writes. A
/// receiver does not identify the keyboard behind it. `writes` names what would be written,
/// as in `keymap writes`.
pub fn require_write_verified(
    writes: &str,
    device: &KeychronDevice,
) -> Result<WriteVerified, GuardError> {
    if !device.is_receiver && write_verified_model(device.vendor_id, device.product_id).is_some() {
        return Ok(WriteVerified {
            device: device.clone(),
        });
    }
    Err(GuardError::NotWriteVerified {
        writes: writes.to_owned(),
        models: write_verified_models_text(),
        connection: if device.is_receiver { "receiver " } else { "" },
        vendor_id: device.vendor_id,
        product_id: device.product_id,
    })
}
