use openkeychron_keychron::KeychronError;
use openkeychron_via::{MacroError, ViaError};

/// Why a guarded write refused or failed.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum GuardError {
    #[error(
        "{writes} require {models} on a USB cable; receiver and unverified-model writes are \
         refused (got {connection}{vendor_id:04x}:{product_id:04x})"
    )]
    NotWriteVerified {
        writes: String,
        models: String,
        /// `"receiver "` for a receiver, which does not identify the keyboard behind it.
        connection: &'static str,
        vendor_id: u16,
        product_id: u16,
    },

    #[error(
        "device {vendor_id:04x}:{product_id:04x} is not a model verified for writes; supported: \
         {models}"
    )]
    UnverifiedBackupModel {
        vendor_id: u16,
        product_id: u16,
        models: String,
    },

    #[error(transparent)]
    Via(#[from] ViaError),

    #[error(transparent)]
    Keychron(#[from] KeychronError),

    #[error("invalid keymap backup: {0}")]
    InvalidKeymapBackup(String),

    #[error("no physical key at ({row}, {column}) in the supported V5 Max ISO layout")]
    NoPhysicalKey { row: u8, column: u8 },

    #[error("layer {layer} is unavailable; select 0..={last_layer}")]
    LayerUnavailable { layer: u8, last_layer: u8 },

    #[error("target layout/options/layer count do not match the supported backup profile")]
    KeymapProfileMismatch,

    #[error("complete matrix/encoder keymap verification did not match the expected state")]
    KeymapReadBackMismatch,

    #[error("invalid macro backup: {0}")]
    InvalidMacroBackup(String),

    #[error(
        "the keyboard's macro count ({macro_count}) and buffer size ({buffer_size}) do not \
         match the backup"
    )]
    MacroLayoutMismatch { macro_count: u8, buffer_size: u16 },

    #[error(
        "the macro buffer's last byte is not 0, so an earlier write did not finish and its \
         content is unknown; restore a macro backup first"
    )]
    MacrosDisabled,

    #[error("the keyboard reports no macro slots")]
    NoMacroSlots,

    #[error("macro slot {slot} is unavailable; select 0..={last_slot}")]
    MacroSlotUnavailable { slot: u8, last_slot: u8 },

    #[error(transparent)]
    Macro(#[from] MacroError),

    #[error("macro buffer verification did not match the expected state")]
    MacroReadBackMismatch,

    #[error(
        "the macro backup records an unfinished write (last byte not 0); restoring it would \
         leave every macro disabled"
    )]
    RestoredMacrosDisabled,
}

/// A guarded write that failed, and whether the keyboard may have changed. Its message is the
/// error's alone.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum GuardedWriteError<Backup, E> {
    /// The capture or `keep` failed, so no write was sent.
    #[error("{0}")]
    NothingWritten(E),
    /// `keep` stored `backup`, then the write or its read-back failed: the keyboard may have
    /// changed, and restoring `backup` undoes it. Boxed, because a whole keymap is large.
    #[error("{error}")]
    AfterBackup { error: E, backup: Box<Backup> },
}

impl<Backup, E> GuardedWriteError<Backup, E> {
    /// The error alone, for a caller that already holds the backup `keep` stored.
    pub fn into_error(self) -> E {
        match self {
            Self::NothingWritten(error) | Self::AfterBackup { error, .. } => error,
        }
    }
}
