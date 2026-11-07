//! The order `rgb set` and `rgb key` keep, by type: the capture step gets only a read-only
//! wrapper, the previous values are saved to the backup file, and only then does the write step
//! get the client with `set_rgb_value`, `set_color` and `save`. Unlike [`crate::guarded_write`],
//! the backup is plain JSON and every message but the shared backup refusal is `rgb`'s own. The
//! backup goes to a new file, so following the printed restore command with the same `--backup`
//! cannot replace the previous values it holds.

use std::path::Path;

use openkeychron_keychron::{Keychron, KeychronError, MatrixSize, PerKeyColor, PerKeyRgb};
use openkeychron_via::{RgbMatrix, Via, ViaError};
use serde::Serialize;

use crate::backup_file;
use crate::session::{Session, SessionError};

/// The VIA lighting read `rgb set` needs before its backup, and no write.
pub(crate) struct LightingReader<'a>(&'a mut Via<Session>);

impl LightingReader<'_> {
    pub(crate) fn rgb_matrix(&mut self) -> Result<RgbMatrix, ViaError> {
        self.0.rgb_matrix()
    }
}

/// The per-key reads `rgb key` needs before its backup, and no write.
pub(crate) struct PerKeyReader<'a, 'keychron>(&'a mut PerKeyRgb<'keychron, Session>);

impl PerKeyReader<'_, '_> {
    pub(crate) fn led_index(
        &mut self,
        matrix: MatrixSize,
        row: u8,
        column: u8,
    ) -> Result<Option<u8>, KeychronError> {
        self.0.led_index(matrix, row, column)
    }

    pub(crate) fn color(&mut self, led: u8) -> Result<PerKeyColor, KeychronError> {
        self.0.color(led)
    }
}

/// Runs a VIA lighting write on `session`: `capture` reads the previous values, they are saved to
/// `backup`, and only then does `write` get the [`Via`] and those values. A failure returns
/// before the session is finished, as `rgb set` always has.
pub(crate) fn write_lighting<Previous: Serialize>(
    session: Session,
    backup: &Path,
    capture: impl FnOnce(&mut LightingReader<'_>) -> Result<Previous, SessionError>,
    write: impl FnOnce(&mut Via<Session>, Previous) -> Result<(), SessionError>,
) -> Result<(), SessionError> {
    let mut via = Via::new(session);
    let previous = capture(&mut LightingReader(&mut via))?;
    backup_file::save_new(backup, "RGB", &previous)?;
    write(&mut via, previous)?;
    via.into_transport().finish()
}

/// Runs a per-key RGB write on `session`: `capture` reads the previous colour, it is saved to
/// `backup`, and only then does `write` get the [`PerKeyRgb`] and that colour. The session is
/// ended whatever the outcome.
pub(crate) fn write_per_key<Previous: Serialize>(
    session: Session,
    backup: &Path,
    capture: impl FnOnce(&mut PerKeyReader<'_, '_>) -> Result<Previous, SessionError>,
    write: impl FnOnce(&mut PerKeyRgb<'_, Session>, Previous) -> Result<(), SessionError>,
) -> Result<(), SessionError> {
    let mut keychron = Keychron::new(session);
    let result = keychron
        .per_key_rgb()
        .map_err(SessionError::from)
        .and_then(|mut rgb| {
            let previous = capture(&mut PerKeyReader(&mut rgb))?;
            backup_file::save_new(backup, "per-key RGB", &previous)?;
            write(&mut rgb, previous)
        });
    keychron.into_transport().complete(result)
}
