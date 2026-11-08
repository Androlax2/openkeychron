#[cfg(test)]
mod tests {
    use openkeychron_guard::keymap::{KeyPosition, KeymapBackup, reset, restore, set_keycode};
    use openkeychron_guard::{GuardError, GuardedWriteError};
    use openkeychron_hid::ReplayTransport;
    use openkeychron_via::Via;

    use crate::support::{
        TestError, assert_stopped_after, backup_json, first_events, fixture, keep_nothing,
        refuse_to_keep, verified,
    };

    /// `keymap set` reads layer count, layout options, the whole matrix and both encoder
    /// directions of every layer before its backup: 26 events.
    const CAPTURE_EVENTS: usize = 26;

    fn original_backup() -> KeymapBackup {
        backup_json("keymap-backup.json")
    }

    fn key_2_1() -> KeyPosition {
        KeyPosition::new(2, 1).unwrap()
    }

    #[test]
    fn set_keycode_returns_the_keymap_from_before_the_change() {
        let mut via = Via::new(ReplayTransport::new(fixture("cli-keymap-set.toml")));
        let backup = set_keycode(&mut via, &verified(), 0, key_2_1(), 0x0004, keep_nothing);
        assert_eq!(backup, Ok(original_backup()));
        via.into_transport().finish().unwrap();
    }

    #[test]
    fn set_keycode_hands_keep_the_backup_it_returns() {
        let mut via = Via::new(ReplayTransport::new(fixture("cli-keymap-set.toml")));
        let mut kept = None;
        let returned = set_keycode(&mut via, &verified(), 0, key_2_1(), 0x0004, |backup| {
            kept = Some(backup.clone());
            Ok::<_, TestError>(())
        })
        .unwrap();
        assert_eq!(kept, Some(returned));
    }

    #[test]
    fn set_keycode_sends_no_write_when_the_backup_is_not_kept() {
        let mut via = Via::new(ReplayTransport::new(fixture("cli-keymap-set.toml")));
        let result = set_keycode(&mut via, &verified(), 0, key_2_1(), 0x0004, refuse_to_keep);
        assert_eq!(
            result,
            Err(GuardedWriteError::NothingWritten(TestError::NotKept))
        );
        assert_stopped_after(via.into_transport(), "cli-keymap-set.toml", CAPTURE_EVENTS);
    }

    #[test]
    fn set_keycode_returns_the_kept_backup_when_the_write_fails() {
        let mut via = Via::new(first_events("cli-keymap-set.toml", CAPTURE_EVENTS + 1));
        let result = set_keycode(&mut via, &verified(), 0, key_2_1(), 0x0004, keep_nothing);
        let Err(GuardedWriteError::AfterBackup { backup, .. }) = result else {
            panic!("expected a failure after the backup, got {result:?}");
        };
        assert_eq!(*backup, original_backup());
    }

    #[test]
    fn set_keycode_refuses_a_missing_layer_before_the_backup() {
        let mut via = Via::new(first_events("cli-keymap-set.toml", CAPTURE_EVENTS));
        let result = set_keycode(&mut via, &verified(), 9, key_2_1(), 0x0004, refuse_to_keep);
        assert!(
            matches!(
                result,
                Err(GuardedWriteError::NothingWritten(TestError::Guard(
                    GuardError::LayerUnavailable { layer: 9, .. }
                )))
            ),
            "{result:?}"
        );
    }

    #[test]
    fn key_position_refuses_a_matrix_slot_without_a_physical_key() {
        assert_eq!(
            KeyPosition::new(0, 1),
            Err(GuardError::NoPhysicalKey { row: 0, column: 1 })
        );
    }

    #[test]
    fn restore_writes_the_backup_and_returns_the_keymap_it_replaced() {
        let mut via = Via::new(ReplayTransport::new(fixture("cli-keymap-restore.toml")));
        let replaced = restore(&mut via, &verified(), &original_backup(), keep_nothing).unwrap();
        assert_eq!(
            (replaced.matrix[1], replaced.encoders[0].clockwise),
            (0xffff, 0x4444)
        );
        via.into_transport().finish().unwrap();
    }

    #[test]
    fn restore_refuses_an_invalid_backup_before_any_report() {
        let mut wanted = original_backup();
        wanted.format_version = 2;
        let mut via = Via::new(first_events("cli-keymap-restore.toml", 0));
        let result = restore(&mut via, &verified(), &wanted, keep_nothing);
        assert_eq!(
            result,
            Err(GuardedWriteError::NothingWritten(TestError::Guard(
                GuardError::InvalidKeymapBackup("unsupported format_version".to_owned())
            )))
        );
    }

    #[test]
    fn reset_returns_the_keymap_from_before_the_reset() {
        let mut via = Via::new(ReplayTransport::new(fixture("cli-keymap-reset.toml")));
        let previous = reset(&mut via, &verified(), keep_nothing);
        assert_eq!(previous, Ok(original_backup()));
        via.into_transport().finish().unwrap();
    }
}
