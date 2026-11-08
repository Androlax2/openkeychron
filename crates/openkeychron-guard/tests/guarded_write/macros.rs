#[cfg(test)]
mod tests {
    use openkeychron_guard::macros::{MacroBackup, restore, set_macro};
    use openkeychron_guard::{GuardError, GuardedWriteError};
    use openkeychron_hid::{Event, ReplayTransport};
    use openkeychron_via::{MacroAction, Via, encode_macro};

    use crate::support::{
        TestError, assert_stopped_after, backup_json, first_events, fixture, keep_nothing,
        refuse_to_keep, verified,
    };

    /// Macro count, buffer size and the whole 40-byte buffer in two reads: 8 events.
    const CAPTURE_EVENTS: usize = 8;

    fn original_backup() -> MacroBackup {
        backup_json("macro-backup.json")
    }

    /// What `cli-macro-set.toml` writes to slot 1.
    fn new_macro() -> Vec<u8> {
        encode_macro(&[
            MacroAction::Text("hi".to_owned()),
            MacroAction::Tap(0x2c),
            MacroAction::Delay(50),
        ])
        .unwrap()
    }

    #[test]
    fn set_macro_returns_the_buffer_from_before_the_change() {
        let mut via = Via::new(ReplayTransport::new(fixture("cli-macro-set.toml")));
        let backup = set_macro(&mut via, &verified(), 1, &new_macro(), keep_nothing);
        assert_eq!(backup, Ok(original_backup()));
        via.into_transport().finish().unwrap();
    }

    #[test]
    fn set_macro_sends_no_write_when_the_backup_is_not_kept() {
        let mut via = Via::new(ReplayTransport::new(fixture("cli-macro-set.toml")));
        let result = set_macro(&mut via, &verified(), 1, &new_macro(), refuse_to_keep);
        assert_eq!(
            result,
            Err(GuardedWriteError::NothingWritten(TestError::NotKept))
        );
        assert_stopped_after(via.into_transport(), "cli-macro-set.toml", CAPTURE_EVENTS);
    }

    #[test]
    fn set_macro_returns_the_kept_backup_when_the_read_back_differs() {
        let mut session = fixture("cli-macro-set.toml");
        let Some(Event::Read { data }) = session.events.last_mut() else {
            panic!("the last event is the read-back of the buffer's tail");
        };
        data[4] ^= 0xff;
        let mut via = Via::new(ReplayTransport::new(session));
        let result = set_macro(&mut via, &verified(), 1, &new_macro(), keep_nothing);
        assert_eq!(
            result,
            Err(GuardedWriteError::AfterBackup {
                error: TestError::Guard(GuardError::MacroReadBackMismatch),
                backup: Box::new(original_backup()),
            })
        );
    }

    #[test]
    fn set_macro_refuses_a_buffer_an_unfinished_write_left_disabled() {
        let mut session = fixture("cli-macro-set.toml");
        session.events.truncate(CAPTURE_EVENTS);
        let Event::Read { data } = &mut session.events[7] else {
            panic!("event 7 is the second buffer read");
        };
        data[4 + 11] = 0xff;
        let mut via = Via::new(ReplayTransport::new(session));
        let result = set_macro(&mut via, &verified(), 1, &new_macro(), keep_nothing);
        assert_eq!(
            result,
            Err(GuardedWriteError::NothingWritten(TestError::Guard(
                GuardError::MacrosDisabled
            )))
        );
    }

    #[test]
    fn restore_writes_the_backup_and_returns_the_buffer_it_replaced() {
        let mut via = Via::new(ReplayTransport::new(fixture("cli-macro-restore.toml")));
        let replaced = restore(&mut via, &verified(), &original_backup(), keep_nothing).unwrap();
        assert_eq!(replaced.buffer[9], b'h');
        via.into_transport().finish().unwrap();
    }

    #[test]
    fn restore_refuses_a_backup_that_would_leave_every_macro_disabled() {
        let mut wanted = original_backup();
        *wanted.buffer.last_mut().unwrap() = 1;
        let mut via = Via::new(first_events("cli-macro-restore.toml", 0));
        let result = restore(&mut via, &verified(), &wanted, keep_nothing);
        assert_eq!(
            result,
            Err(GuardedWriteError::NothingWritten(TestError::Guard(
                GuardError::RestoredMacrosDisabled
            )))
        );
    }

    #[test]
    fn restore_sends_no_write_when_the_backup_is_not_kept() {
        let mut via = Via::new(ReplayTransport::new(fixture("cli-macro-restore.toml")));
        let result = restore(&mut via, &verified(), &original_backup(), refuse_to_keep);
        assert_eq!(
            result,
            Err(GuardedWriteError::NothingWritten(TestError::NotKept))
        );
        assert_stopped_after(
            via.into_transport(),
            "cli-macro-restore.toml",
            CAPTURE_EVENTS,
        );
    }

    #[test]
    fn restore_refuses_a_keyboard_with_another_macro_count() {
        let mut wanted = original_backup();
        wanted.macro_count += 1;
        let mut via = Via::new(ReplayTransport::new(fixture("cli-macro-restore.toml")));
        let result = restore(&mut via, &verified(), &wanted, keep_nothing);
        assert!(
            matches!(
                result,
                Err(GuardedWriteError::NothingWritten(TestError::Guard(
                    GuardError::MacroLayoutMismatch { .. }
                )))
            ),
            "{result:?}"
        );
    }
}
