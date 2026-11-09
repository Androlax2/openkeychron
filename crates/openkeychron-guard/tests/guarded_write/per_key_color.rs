//! `set_key_color` inside a per-key write: the previous colour is set again when the new one does
//! not read back, whether it reads back different or the read-back gets no reply, and a colour is
//! saved only when it did.

#[cfg(test)]
mod tests {
    use openkeychron_guard::rgb::{set_key_color, write_per_key};
    use openkeychron_guard::{GuardError, GuardedWriteError};
    use openkeychron_hid::{ReplayTransport, TransportError};
    use openkeychron_keychron::{Keychron, KeychronError, MatrixSize, PerKeyColor};
    use openkeychron_via::V5_MAX_FR_ISO;

    use crate::support::{TestError, first_events, fixture, keep_nothing, verified};

    const V5_MAX_MATRIX: MatrixSize = MatrixSize {
        rows: V5_MAX_FR_ISO.rows,
        columns: V5_MAX_FR_ISO.columns,
    };

    /// The LED under the `A` key (matrix 2,1) and the colour it had before the write.
    type KeyBackup = (u8, PerKeyColor);

    const PREVIOUS_COLOR: PerKeyColor = PerKeyColor {
        hue: 170,
        saturation: 255,
        value: 255,
    };

    fn set_key_a_to_red(
        transport: ReplayTransport,
    ) -> (
        Result<KeyBackup, GuardedWriteError<KeyBackup, TestError>>,
        ReplayTransport,
    ) {
        let mut keychron = Keychron::new(transport);
        let result = write_per_key(
            &mut keychron,
            &verified(),
            |reader| {
                let led = reader.led_index(V5_MAX_MATRIX, 2, 1)?.unwrap();
                Ok(((led, reader.color(led)?), ()))
            },
            keep_nothing,
            |rgb, &(led, previous), ()| Ok(set_key_color(rgb, led, previous, 0, 255)?),
        );
        (result, keychron.into_transport())
    }

    fn mismatch_on_read_back() -> KeychronError {
        KeychronError::ReadBackMismatch {
            written: "LED 38: PerKeyColor { hue: 0, saturation: 255, value: 255 }".to_owned(),
            read: "LED 38: PerKeyColor { hue: 85, saturation: 255, value: 255 }".to_owned(),
        }
    }

    #[test]
    fn saves_a_colour_that_reads_back() {
        let (result, transport) =
            set_key_a_to_red(ReplayTransport::new(fixture("cli-rgb-key.toml")));
        assert_eq!(result, Ok((38, PREVIOUS_COLOR)));
        // The whole fixture, which ends with the save.
        transport.finish().unwrap();
    }

    #[test]
    fn sets_the_previous_colour_again_and_saves_nothing_when_the_colour_does_not_read_back() {
        let (result, transport) =
            set_key_a_to_red(ReplayTransport::new(fixture("cli-rgb-key-mismatch.toml")));
        assert_eq!(
            result,
            Err(GuardedWriteError::AfterBackup {
                error: TestError::Guard(GuardError::PerKeyColorRolledBack {
                    failure: mismatch_on_read_back()
                }),
                backup: Box::new((38, PREVIOUS_COLOR)),
            })
        );
        // The fixture holds the second write and its read-back, and ends before any save.
        transport.finish().unwrap();
    }

    #[test]
    fn sets_the_previous_colour_again_and_saves_nothing_when_the_read_back_gets_no_reply() {
        let (result, transport) = set_key_a_to_red(ReplayTransport::new(fixture(
            "cli-rgb-key-read-back-timeout.toml",
        )));
        assert_eq!(
            result,
            Err(GuardedWriteError::AfterBackup {
                error: TestError::Guard(GuardError::PerKeyColorRolledBack {
                    failure: KeychronError::Transport(TransportError::Timeout),
                }),
                backup: Box::new((38, PREVIOUS_COLOR)),
            })
        );
        // The fixture holds the second write and its read-back, and ends before any save.
        transport.finish().unwrap();
    }

    #[test]
    fn says_so_when_setting_the_previous_colour_again_also_fails() {
        // Events 0..14 of the fixture end with the mismatching read-back (event 13), so the
        // second write has no answer.
        let transport = first_events("cli-rgb-key-mismatch.toml", 14);
        let (result, _) = set_key_a_to_red(transport);
        let Err(GuardedWriteError::AfterBackup {
            error: TestError::Guard(GuardError::PerKeyColorRollbackFailed { failure, restore }),
            backup,
        }) = result
        else {
            panic!("expected a failed rollback, got {result:?}");
        };
        assert_eq!(failure, mismatch_on_read_back());
        assert!(
            matches!(
                restore,
                KeychronError::Transport(TransportError::UnexpectedWrite { .. })
            ),
            "{restore:?}"
        );
        assert_eq!(*backup, (38, PREVIOUS_COLOR));
    }

    #[test]
    fn does_not_set_the_previous_colour_again_when_the_first_write_is_not_sent() {
        // Events 0..10 of the fixture end with the capture (event 9), so the replay refuses the
        // first write.
        let transport = first_events("cli-rgb-key-mismatch.toml", 10);
        let (result, _) = set_key_a_to_red(transport);
        assert!(
            matches!(
                result,
                Err(GuardedWriteError::AfterBackup {
                    error: TestError::Guard(GuardError::Keychron(_)),
                    ..
                })
            ),
            "{result:?}"
        );
    }
}
