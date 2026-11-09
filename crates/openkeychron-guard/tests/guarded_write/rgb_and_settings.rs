//! Each RGB and settings write replays its whole fixture with a `keep` that fails: the session
//! must stop right after the reads the write makes before its backup.

#[cfg(test)]
mod tests {
    use openkeychron_guard::GuardedWriteError;
    use openkeychron_guard::rgb::{write_lighting, write_mixed, write_per_key};
    use openkeychron_guard::settings::write_settings;
    use openkeychron_hid::ReplayTransport;
    use openkeychron_keychron::{Debounce, DebounceAlgorithm, Keychron, MatrixSize, PerKeyColor};
    use openkeychron_via::{RgbValue, V5_MAX_FR_ISO, Via};

    use crate::support::{
        TestError, assert_stopped_after, first_events, fixture, keep_nothing, refuse_to_keep,
        verified,
    };

    const V5_MAX_MATRIX: MatrixSize = MatrixSize {
        rows: V5_MAX_FR_ISO.rows,
        columns: V5_MAX_FR_ISO.columns,
    };

    fn not_kept<B: std::fmt::Debug>(result: &Result<B, GuardedWriteError<B, TestError>>) {
        assert!(
            matches!(
                result,
                Err(GuardedWriteError::NothingWritten(TestError::NotKept))
            ),
            "{result:?}"
        );
    }

    #[test]
    fn lighting_sends_no_write_when_the_backup_is_not_kept() {
        let mut via = Via::new(ReplayTransport::new(fixture("cli-rgb-set-brightness.toml")));
        not_kept(&write_lighting(
            &mut via,
            &verified(),
            |reader| Ok((reader.rgb_matrix()?, ())),
            refuse_to_keep,
            |via, _, ()| Ok(via.set_rgb_value(RgbValue::Brightness(128))?),
        ));
        // Brightness, effect, speed and colour: four reads.
        assert_stopped_after(via.into_transport(), "cli-rgb-set-brightness.toml", 8);
    }

    #[test]
    fn per_key_sends_no_write_when_the_backup_is_not_kept() {
        let mut keychron = Keychron::new(ReplayTransport::new(fixture("cli-rgb-key.toml")));
        not_kept(&write_per_key(
            &mut keychron,
            &verified(),
            |reader| {
                let led = reader.led_index(V5_MAX_MATRIX, 2, 1)?.unwrap();
                Ok(((led, reader.color(led)?), ()))
            },
            refuse_to_keep,
            |rgb, &(led, _), ()| {
                let wanted = PerKeyColor {
                    hue: 0,
                    saturation: 255,
                    value: 255,
                };
                Ok(rgb.set_color(led, wanted)?)
            },
        ));
        // As many as cli-rgb-key-before-backup.toml holds.
        assert_stopped_after(keychron.into_transport(), "cli-rgb-key.toml", 10);
    }

    #[test]
    fn mixed_sends_no_write_when_the_backup_is_not_kept() {
        let mut keychron =
            Keychron::new(ReplayTransport::new(fixture("cli-rgb-mixed-region.toml")));
        not_kept(&write_mixed(
            &mut keychron,
            &verified(),
            |reader| {
                reader.firmware_version()?.unwrap();
                let regions = reader.regions()?;
                for region in 0..reader.region_count() {
                    reader.effects(region)?;
                }
                for row in [2, 4] {
                    reader.leds_in_row(V5_MAX_MATRIX, row)?;
                }
                Ok((regions, ()))
            },
            refuse_to_keep,
            |rgb, regions, ()| Ok(rgb.set_regions(0, regions)?),
        ));
        // As many as cli-rgb-mixed-region-before-backup.toml holds.
        assert_stopped_after(keychron.into_transport(), "cli-rgb-mixed-region.toml", 32);
    }

    fn set_debounce_to_10_ms(keychron: &mut Keychron<ReplayTransport>) -> Result<(), TestError> {
        Ok(keychron.set_debounce(Debounce {
            algorithm: DebounceAlgorithm::SymmetricEagerPerKey,
            time_ms: 10,
        })?)
    }

    #[test]
    fn settings_send_no_write_when_the_backup_is_not_kept() {
        let mut keychron = Keychron::new(ReplayTransport::new(fixture(
            "cli-settings-set-debounce.toml",
        )));
        not_kept(&write_settings(
            &mut keychron,
            &verified(),
            |reader| Ok((reader.debounce()?, ())),
            refuse_to_keep,
            |keychron, _, ()| set_debounce_to_10_ms(keychron),
        ));
        // As many as cli-settings-set-debounce-reads.toml holds.
        assert_stopped_after(
            keychron.into_transport(),
            "cli-settings-set-debounce.toml",
            4,
        );
    }

    #[test]
    fn settings_return_the_value_from_before_the_change() {
        let mut keychron = Keychron::new(ReplayTransport::new(fixture(
            "cli-settings-set-debounce.toml",
        )));
        let previous = write_settings(
            &mut keychron,
            &verified(),
            |reader| Ok((reader.debounce()?, ())),
            keep_nothing,
            |keychron, _, ()| set_debounce_to_10_ms(keychron),
        );
        assert_eq!(
            previous,
            Ok(Debounce {
                algorithm: DebounceAlgorithm::SymmetricDeferGlobal,
                time_ms: 50,
            })
        );
        keychron.into_transport().finish().unwrap();
    }

    #[test]
    fn a_capture_failure_writes_nothing_and_skips_keep() {
        let mut via = Via::new(first_events("cli-rgb-set-brightness.toml", 0));
        let mut keep_ran = false;
        let result = write_lighting(
            &mut via,
            &verified(),
            |reader| Ok((reader.rgb_matrix()?, ())),
            |_| {
                keep_ran = true;
                Ok::<_, TestError>(())
            },
            |_, _, ()| Ok(()),
        );
        assert!(
            matches!(
                result,
                Err(GuardedWriteError::NothingWritten(TestError::Other(_)))
            ),
            "{result:?}"
        );
        assert!(!keep_ran);
    }
}
