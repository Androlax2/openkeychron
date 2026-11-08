//! Guarded RGB writes: VIA lighting, Keychron per-key colours and Keychron mixed RGB. Each capture
//! step gets only a read-only wrapper; the write step gets the client with `set_rgb_value`,
//! `set_color`, `set_regions`, `set_effects` and `save` once the backup is kept.

use openkeychron_hid::HidTransport;
use openkeychron_keychron::{
    FirmwareVersion, Keychron, KeychronError, MatrixSize, MixedEffect, MixedRgb, NotReported,
    PerKeyColor, PerKeyRgb,
};
use openkeychron_via::{RgbMatrix, Via, ViaError};

use crate::{GuardError, GuardedWriteError, WriteVerified, keep_then_write};

/// The VIA lighting read a lighting write needs before its backup, and no write.
pub struct LightingReader<'a, T>(&'a mut Via<T>);

impl<T: HidTransport> LightingReader<'_, T> {
    pub fn rgb_matrix(&mut self) -> Result<RgbMatrix, ViaError> {
        self.0.rgb_matrix()
    }
}

/// Runs a VIA lighting write: `capture` reads the previous values, `keep` stores them, and only
/// then does `write` get the [`Via`]. Returns the previous values.
pub fn write_lighting<T, Backup, Plan, E>(
    via: &mut Via<T>,
    _verified: &WriteVerified,
    capture: impl FnOnce(&mut LightingReader<'_, T>) -> Result<(Backup, Plan), E>,
    keep: impl FnOnce(&Backup) -> Result<(), E>,
    write: impl FnOnce(&mut Via<T>, &Backup, Plan) -> Result<(), E>,
) -> Result<Backup, GuardedWriteError<Backup, E>>
where
    T: HidTransport,
{
    let (backup, plan) =
        capture(&mut LightingReader(via)).map_err(GuardedWriteError::NothingWritten)?;
    keep_then_write(backup, keep, |backup| write(via, backup, plan))
}

/// The per-key reads a per-key write needs before its backup, and no write.
pub struct PerKeyReader<'a, 'keychron, T>(&'a mut PerKeyRgb<'keychron, T>);

impl<T: HidTransport> PerKeyReader<'_, '_, T> {
    pub fn led_index(
        &mut self,
        matrix: MatrixSize,
        row: u8,
        column: u8,
    ) -> Result<Option<u8>, KeychronError> {
        self.0.led_index(matrix, row, column)
    }

    pub fn color(&mut self, led: u8) -> Result<PerKeyColor, KeychronError> {
        self.0.color(led)
    }
}

/// Runs a per-key RGB write: `capture` reads the previous colours, `keep` stores them, and only
/// then does `write` get the [`PerKeyRgb`]. Returns the previous colours.
pub fn write_per_key<T, Backup, Plan, E>(
    keychron: &mut Keychron<T>,
    _verified: &WriteVerified,
    capture: impl FnOnce(&mut PerKeyReader<'_, '_, T>) -> Result<(Backup, Plan), E>,
    keep: impl FnOnce(&Backup) -> Result<(), E>,
    write: impl FnOnce(&mut PerKeyRgb<'_, T>, &Backup, Plan) -> Result<(), E>,
) -> Result<Backup, GuardedWriteError<Backup, E>>
where
    T: HidTransport,
    E: From<GuardError>,
{
    let mut rgb = keychron
        .per_key_rgb()
        .map_err(|error| GuardedWriteError::NothingWritten(GuardError::from(error).into()))?;
    let (backup, plan) =
        capture(&mut PerKeyReader(&mut rgb)).map_err(GuardedWriteError::NothingWritten)?;
    keep_then_write(backup, keep, |backup| write(&mut rgb, backup, plan))
}

/// The mixed RGB reads a mixed RGB write needs before its backup, and no write.
pub struct MixedReader<'a, 'keychron, T>(&'a mut MixedRgb<'keychron, T>);

impl<T: HidTransport> MixedReader<'_, '_, T> {
    #[must_use]
    pub fn led_count(&self) -> u8 {
        self.0.led_count()
    }

    #[must_use]
    pub fn region_count(&self) -> u8 {
        self.0.region_count()
    }

    #[must_use]
    pub fn effects_per_region(&self) -> u8 {
        self.0.effects_per_region()
    }

    pub fn firmware_version(
        &mut self,
    ) -> Result<Result<FirmwareVersion, NotReported>, KeychronError> {
        self.0.firmware_version()
    }

    pub fn regions(&mut self) -> Result<Vec<u8>, KeychronError> {
        self.0.regions()
    }

    pub fn effects(&mut self, region: u8) -> Result<Vec<MixedEffect>, KeychronError> {
        self.0.effects(region)
    }

    pub fn leds_in_row(
        &mut self,
        matrix: MatrixSize,
        row: u8,
    ) -> Result<Vec<Option<u8>>, KeychronError> {
        self.0.leds_in_row(matrix, row)
    }
}

/// Runs a mixed RGB write: `capture` reads the previous state and plans the change, `keep`
/// stores the previous state, and only then does `write` get the [`MixedRgb`]. Returns the
/// previous state.
pub fn write_mixed<T, Backup, Plan, E>(
    keychron: &mut Keychron<T>,
    _verified: &WriteVerified,
    capture: impl FnOnce(&mut MixedReader<'_, '_, T>) -> Result<(Backup, Plan), E>,
    keep: impl FnOnce(&Backup) -> Result<(), E>,
    write: impl FnOnce(&mut MixedRgb<'_, T>, &Backup, Plan) -> Result<(), E>,
) -> Result<Backup, GuardedWriteError<Backup, E>>
where
    T: HidTransport,
    E: From<GuardError>,
{
    let mut rgb = keychron
        .mixed_rgb()
        .map_err(|error| GuardedWriteError::NothingWritten(GuardError::from(error).into()))?;
    let (backup, plan) =
        capture(&mut MixedReader(&mut rgb)).map_err(GuardedWriteError::NothingWritten)?;
    keep_then_write(backup, keep, |backup| write(&mut rgb, backup, plan))
}
