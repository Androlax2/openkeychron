//! The one error every command returns. The UI receives it as its message text.

use openkeychron_guard::GuardError;
use openkeychron_hid::{DiscoveryError, OpenError, TransportError};
use openkeychron_keychron::KeychronError;
use openkeychron_via::ViaError;
use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error(transparent)]
    Discovery(#[from] DiscoveryError),

    #[error(transparent)]
    Open(#[from] OpenError),

    #[error("{path} is not a connected Keychron VIA interface; connected: {connected}")]
    NotConnected { path: String, connected: String },

    /// A keymap history belongs to one plug-in of a keyboard, and the app could not tell which
    /// one is at `path`.
    #[error(
        "cannot tell which keyboard is at {path}, so keymap commands on it are refused: \
         {reason}; plug the keyboard in again"
    )]
    UnknownConnection { path: String, reason: String },

    #[error("{0}")]
    Via(String),

    #[error(
        "{reading} needs the keyboard on a USB cable: a receiver does not identify the keyboard \
         behind it"
    )]
    NeedsCable { reading: &'static str },

    #[error(
        "this build has no keyboard layout for {vendor_id:04x}:{product_id:04x}, so it cannot \
         place its keys"
    )]
    NoLayout { vendor_id: u16, product_id: u16 },

    #[error(transparent)]
    Keychron(#[from] KeychronError),

    /// A write refused, or failed before anything was sent. Boxed, like the next variant,
    /// because a `GuardError` is several times larger than every other variant.
    #[error(transparent)]
    Guard(Box<GuardError>),

    /// A write failed after its backup was kept, so the keyboard may have changed. The likeliest
    /// cause is an unplug, and a keyboard plugged in again gets a new history without that
    /// backup, so `recovery` holds only while it stays plugged in.
    #[error(
        "{error}; the keyboard may have changed: {recovery} while the keyboard stays plugged in; \
         the app starts a new history once it is plugged in again"
    )]
    WriteFailedAfterBackup {
        error: Box<CommandError>,
        recovery: &'static str,
    },

    #[error(
        "nothing left to undo: Undo takes back only the keymap changes the app made since it \
         started"
    )]
    NothingToUndo,

    #[error(
        "nothing left to redo: Redo puts back only what Undo took back since the app's last \
         change"
    )]
    NothingToRedo,

    /// `write` names the write refused: Undo or Redo.
    #[error(
        "the keymap is not as the app left it, so {write} would also take back changes made \
         outside the app; nothing was written. Restore backup still puts back the keymap from \
         before the app's first change"
    )]
    KeymapChangedOutsideApp { write: &'static str },

    #[error("no keymap backup yet: the app keeps one when it first changes this keyboard's keymap")]
    NoKeymapBackup,
}

impl From<GuardError> for CommandError {
    fn from(error: GuardError) -> Self {
        Self::Guard(Box::new(error))
    }
}

impl CommandError {
    /// Adds the receiver caveat to a timeout: through the Keychron Link, a timeout alone does
    /// not show why nothing answered (docs/protocol/keychron-link.md).
    pub fn via(error: &ViaError, through_receiver: bool) -> Self {
        let timed_out = *error == ViaError::Transport(TransportError::Timeout);
        if timed_out && through_receiver {
            Self::Via(format!(
                "{error}. Through a receiver this is inconclusive; try the keyboard on a USB \
                 cable (see docs/protocol/keychron-link.md)"
            ))
        } else {
            Self::Via(error.to_string())
        }
    }
}

/// Tauri hands a command's error to the UI as JSON; the message is all the UI shows.
impl Serialize for CommandError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::CommandError;

    #[test]
    fn an_error_reaches_the_ui_as_its_message() {
        let error = CommandError::NeedsCable {
            reading: "the keymap",
        };
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            serde_json::Value::String(error.to_string())
        );
    }
}
