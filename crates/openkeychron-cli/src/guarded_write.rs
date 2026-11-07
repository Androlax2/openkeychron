//! What every backup-guarded keymap and macro write shares: the supported-keyboard check
//! (which `rgb` and `settings` writes use too), the session around the write, and the failure
//! and success messages naming the backup.

use std::path::Path;

use openkeychron_hid::KeychronDevice;
use openkeychron_via::{
    DeviceRegistry, EncoderDirection, Keycode, RegisteredModel, Via, ViaError, WriteAccess,
};
use serde::Serialize;

use crate::session::{Session, SessionError};

/// The model behind `vendor_id:product_id` when the built-in registry marks it as verified for
/// writes. Only the built-in registry is asked, so a layout file passed with `--layout` can
/// never enable a write.
pub(crate) fn write_verified_model(vendor_id: u16, product_id: u16) -> Option<RegisteredModel> {
    DeviceRegistry::built_in()
        .model(vendor_id, product_id)
        .filter(|model| model.write_access() == WriteAccess::VerifiedOverCable)
        .cloned()
}

/// The verified models as `the V5 Max ISO (3434:0951)`, for refusal messages.
pub(crate) fn write_verified_models_text() -> String {
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
pub(crate) fn require_write_verified(
    writes: &str,
    device: &KeychronDevice,
) -> Result<(), SessionError> {
    if !device.is_receiver && write_verified_model(device.vendor_id, device.product_id).is_some() {
        return Ok(());
    }
    let connection = if device.is_receiver { "receiver " } else { "" };
    Err(SessionError::Usage(format!(
        "{writes} require {models} on a USB cable; receiver and unverified-model writes are \
         refused (got {connection}{vendor_id:04x}:{product_id:04x})",
        models = write_verified_models_text(),
        vendor_id = device.vendor_id,
        product_id = device.product_id,
    )))
}

/// The wording of one kind of backup-guarded write.
pub(crate) struct WriteKind {
    /// The command that writes and restores this state: `keymap` or `macro`.
    pub(crate) command: &'static str,
    /// What is written, for [`require_write_verified`]: `keymap writes`.
    pub(crate) writes: &'static str,
    /// What a write that fails after the backup can leave behind.
    pub(crate) failure_risk: &'static str,
    /// What the pre-change backup holds.
    pub(crate) backup_contents: &'static str,
}

/// The pre-change state a write saves to a new file before it sends anything.
pub(crate) trait PreChangeSnapshot {
    /// Saves to `path`, which must not exist yet.
    fn save_new(&self, path: &Path) -> Result<(), SessionError>;
}

/// The reads a capture needs and no write: [`run`] lends the capture step only this, so no
/// write can reach the keyboard before the pre-change backup is saved.
pub(crate) struct ViaReader<'a>(&'a mut Via<Session>);

impl<'a> ViaReader<'a> {
    pub(crate) fn new(via: &'a mut Via<Session>) -> Self {
        Self(via)
    }

    pub(crate) fn layer_count(&mut self) -> Result<u8, ViaError> {
        self.0.layer_count()
    }

    pub(crate) fn layout_options(&mut self) -> Result<u32, ViaError> {
        self.0.layout_options()
    }

    pub(crate) fn keymap(&mut self, key_count: usize) -> Result<Vec<Keycode>, ViaError> {
        self.0.keymap(key_count)
    }

    pub(crate) fn encoder_keycode(
        &mut self,
        layer: u8,
        encoder: u8,
        direction: EncoderDirection,
    ) -> Result<Keycode, ViaError> {
        self.0.encoder_keycode(layer, encoder, direction)
    }

    pub(crate) fn macro_count(&mut self) -> Result<u8, ViaError> {
        self.0.macro_count()
    }

    pub(crate) fn macro_buffer_size(&mut self) -> Result<u16, ViaError> {
        self.0.macro_buffer_size()
    }

    pub(crate) fn macro_buffer(&mut self, size: u16) -> Result<Vec<u8>, ViaError> {
        self.0.macro_buffer(size)
    }
}

/// The new file that receives the pre-change state before the first write is sent.
struct PreChangeBackup<'a> {
    path: &'a Path,
    is_saved: bool,
}

impl PreChangeBackup<'_> {
    /// From a successful save on, a failure names the backup.
    fn save(&mut self, snapshot: &impl PreChangeSnapshot) -> Result<(), SessionError> {
        snapshot.save_new(self.path)?;
        self.is_saved = true;
        Ok(())
    }
}

/// Runs a write against a supported cable keyboard and ends the session. `capture` reads the
/// pre-change snapshot, and anything the write needs, or refuses; `run` saves the snapshot to
/// `backup_path`, and only then hands `write` the [`Via`]. From the save on, every failure,
/// an unfinished replay included, names that backup and the command that restores it.
pub(crate) fn run<Snapshot, Plan, Capture, Write>(
    kind: &WriteKind,
    fixture: Option<&Path>,
    device: Option<&Path>,
    backup_path: &Path,
    capture: Capture,
    write: Write,
) -> Result<(), SessionError>
where
    Snapshot: PreChangeSnapshot,
    Capture: FnOnce(&mut ViaReader<'_>, &KeychronDevice) -> Result<(Snapshot, Plan), SessionError>,
    Write: FnOnce(&mut Via<Session>, &KeychronDevice, Snapshot, Plan) -> Result<(), SessionError>,
{
    let session = Session::start(fixture, None, device)?;
    let identity = session.device().clone();
    let mut via = Via::new(session);
    let mut backup = PreChangeBackup {
        path: backup_path,
        is_saved: false,
    };
    let result = require_write_verified(kind.writes, &identity).and_then(|()| {
        capture(&mut ViaReader::new(&mut via), &identity).and_then(|(snapshot, plan)| {
            backup.save(&snapshot)?;
            write(&mut via, &identity, snapshot, plan)
        })
    });
    via.into_transport()
        .complete(result)
        .map_err(|error| failure(kind, error, &backup))
}

fn failure(kind: &WriteKind, error: SessionError, backup: &PreChangeBackup<'_>) -> SessionError {
    if !backup.is_saved {
        return error;
    }
    let path = backup.path.display();
    SessionError::Usage(format!(
        "{error}; {command} state may have changed, and {risk}; \
         pre-change {contents} backup retained at {path}; \
         undo with `openkeychron {command} restore {path} --backup <NEW_FILE>`",
        command = kind.command,
        risk = kind.failure_risk,
        contents = kind.backup_contents,
    ))
}

#[derive(Debug, Serialize)]
struct SuccessOutput<'a> {
    operation: &'static str,
    backup: &'a Path,
}

/// Prints `summary` with the backup and its undo command, or `operation` and the backup as JSON.
pub(crate) fn print_success(
    kind: &WriteKind,
    operation: &'static str,
    summary: &str,
    backup: &Path,
    json: bool,
) -> Result<(), SessionError> {
    if json {
        return crate::print_json(&SuccessOutput { operation, backup });
    }
    println!("{summary}");
    println!("Pre-change backup: {}", backup.display());
    println!(
        "Undo with: openkeychron {} restore {} --backup <NEW_FILE>",
        kind.command,
        backup.display()
    );
    Ok(())
}
