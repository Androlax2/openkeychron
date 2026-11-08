//! The keymap writes the UI makes: set one key, undo the last change, and restore the keymap
//! from before the first change. Each goes through `openkeychron_guard::keymap`, which refuses a
//! receiver or an unverified model, has this module keep the keymap it replaces in memory before
//! it sends anything, and reads the whole keymap back. This module only decides which backup to
//! keep and which one to write back.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use openkeychron_guard::keymap::{self as guarded_keymap, KeyPosition, KeymapBackup};
use openkeychron_guard::{GuardError, GuardedWriteError, require_write_verified};
use openkeychron_hid::{HidTransport, KeychronDevice};
use openkeychron_via::Via;
use serde::{Deserialize, Serialize};

use crate::error::CommandError;
use crate::keymap::{self, Keymap};

/// What the write gate's refusal names.
const WRITES: &str = "keymap writes";

/// A keymap, and what the app can undo on its device.
#[derive(Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EditableKeymap {
    pub keymap: Keymap,
    /// How many changes Undo can take back, one at a time, newest first.
    pub undo_steps: usize,
    /// True once the app holds the keymap from before its first change on this device.
    pub can_restore_backup: bool,
}

/// One key to set: `layer`, the key's matrix position, and the keycode it gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KeyAssignment {
    pub layer: u8,
    pub row: u8,
    pub column: u8,
    pub keycode: u16,
}

/// Every keymap the app replaced on one device, kept in memory until the app quits.
#[derive(Debug, Default)]
pub struct KeymapHistory {
    /// The keymap from before the app's first change: what Restore backup writes back.
    first_backup: Option<KeymapBackup>,
    /// The keymap from before each change Undo can take back, oldest first. A restore's backup
    /// goes here too, so Undo takes a restore back.
    undo: Vec<KeymapBackup>,
    /// The keymap each undo replaced. Nothing offers it yet (redo is a follow-up); it is kept
    /// because every write keeps what it replaces before it is sent (`protocol.md`).
    undone: Vec<KeymapBackup>,
}

impl KeymapHistory {
    fn with(&self, keymap: Keymap) -> EditableKeymap {
        EditableKeymap {
            keymap,
            undo_steps: self.undo.len(),
            can_restore_backup: self.first_backup.is_some(),
        }
    }
}

/// The keymap history of each device, by the path `list_devices` gave it.
#[derive(Debug, Default)]
pub struct KeymapHistories(Mutex<HashMap<String, KeymapHistory>>);

impl KeymapHistories {
    pub fn lock(&self) -> MutexGuard<'_, HashMap<String, KeymapHistory>> {
        // Each change to a history is one push or pop, so a command that panicked while holding
        // the lock left no history half-changed.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Reads the keymap. Only reads.
pub fn read<T: HidTransport>(
    transport: T,
    device: &KeychronDevice,
    history: &KeymapHistory,
) -> Result<EditableKeymap, CommandError> {
    Ok(history.with(keymap::read(transport, device)?))
}

/// Sets one key, then returns the keymap read again after the write.
pub fn set_keycode<T: HidTransport>(
    transport: T,
    device: &KeychronDevice,
    history: &mut KeymapHistory,
    assignment: KeyAssignment,
) -> Result<EditableKeymap, CommandError> {
    let verified = require_write_verified(WRITES, device)?;
    let key = KeyPosition::new(assignment.row, assignment.column)?;
    let mut via = Via::new(transport);
    guarded_keymap::set_keycode::<_, GuardError>(
        &mut via,
        &verified,
        assignment.layer,
        key,
        assignment.keycode,
        |previous| {
            history.first_backup.get_or_insert_with(|| previous.clone());
            history.undo.push(previous.clone());
            Ok(())
        },
    )
    .map_err(|error| failed(error, "Undo puts back the keymap from before this change"))?;
    read(via.into_transport(), device, history)
}

/// Writes back the keymap from before the newest change Undo can take back.
pub fn undo<T: HidTransport>(
    transport: T,
    device: &KeychronDevice,
    history: &mut KeymapHistory,
) -> Result<EditableKeymap, CommandError> {
    let verified = require_write_verified(WRITES, device)?;
    let previous = history
        .undo
        .last()
        .cloned()
        .ok_or(CommandError::NothingToUndo)?;
    let mut via = Via::new(transport);
    guarded_keymap::restore::<_, GuardError>(&mut via, &verified, &previous, |replaced| {
        history.undone.push(replaced.clone());
        Ok(())
    })
    .map_err(|error| failed(error, "Undo again to retry"))?;
    history.undo.pop();
    read(via.into_transport(), device, history)
}

/// Writes back the keymap from before the app's first change on this device.
pub fn restore_backup<T: HidTransport>(
    transport: T,
    device: &KeychronDevice,
    history: &mut KeymapHistory,
) -> Result<EditableKeymap, CommandError> {
    let verified = require_write_verified(WRITES, device)?;
    let backup = history
        .first_backup
        .clone()
        .ok_or(CommandError::NoKeymapBackup)?;
    let mut via = Via::new(transport);
    guarded_keymap::restore::<_, GuardError>(&mut via, &verified, &backup, |replaced| {
        history.undo.push(replaced.clone());
        Ok(())
    })
    .map_err(|error| {
        failed(
            error,
            "Undo puts back the keymap from before the restore, and Restore backup tries again",
        )
    })?;
    read(via.into_transport(), device, history)
}

/// A failed guarded write. Once the backup is kept, the keyboard may have changed, so the
/// message says how to get back.
fn failed(
    error: GuardedWriteError<KeymapBackup, GuardError>,
    recovery: &'static str,
) -> CommandError {
    match error {
        GuardedWriteError::NothingWritten(error) => error.into(),
        GuardedWriteError::AfterBackup { error, .. } => CommandError::WriteFailedAfterBackup {
            error: Box::new(error),
            recovery,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use openkeychron_hid::{HidTransport, KeychronDevice, TransportError, fixture_device};
    use openkeychron_mock::{MockKeyboard, MockTransport};

    use super::{
        EditableKeymap, KeyAssignment, KeymapHistory, read, restore_backup, set_keycode, undo,
    };
    use crate::error::CommandError;
    use crate::test_support::{Lent, lend, load_fixture, replay, synthetic_path};

    /// Row 2, column 1: the key labelled `A` on the FR ISO layout.
    const ROW: u8 = 2;
    const COLUMN: u8 = 1;
    const COLUMNS: usize = 19;
    const KC_A: u16 = 0x0004;
    const KC_B: u16 = 0x0005;
    const KC_C: u16 = 0x0006;

    struct Mock {
        keyboard: MockKeyboard,
        device: KeychronDevice,
        history: KeymapHistory,
    }

    impl Mock {
        fn new() -> Self {
            let path = synthetic_path("v5-max-iso.mock.toml");
            let keyboard = MockKeyboard::load(&path).unwrap();
            let device = fixture_device(keyboard.identity(), Path::new("mock")).unwrap();
            Self {
                keyboard,
                device,
                history: KeymapHistory::default(),
            }
        }

        fn set(&mut self, layer: u8, keycode: u16) -> EditableKeymap {
            set_keycode(
                self.keyboard.connect(),
                &self.device,
                &mut self.history,
                assignment(layer, keycode),
            )
            .unwrap()
        }

        fn undo(&mut self) -> EditableKeymap {
            undo(self.keyboard.connect(), &self.device, &mut self.history).unwrap()
        }

        fn restore_backup(&mut self) -> EditableKeymap {
            restore_backup(self.keyboard.connect(), &self.device, &mut self.history).unwrap()
        }

        fn read(&self) -> EditableKeymap {
            read(self.keyboard.connect(), &self.device, &self.history).unwrap()
        }
    }

    fn assignment(layer: u8, keycode: u16) -> KeyAssignment {
        KeyAssignment {
            layer,
            row: ROW,
            column: COLUMN,
            keycode,
        }
    }

    fn keycode_at(keymap: &EditableKeymap, layer: usize) -> u16 {
        keymap.keymap.layers[layer].keys[usize::from(ROW) * COLUMNS + usize::from(COLUMN)].keycode
    }

    #[test]
    fn returns_the_keymap_read_back_after_setting_a_key() {
        let mut mock = Mock::new();
        let edited = mock.set(1, KC_A);
        assert_eq!(keycode_at(&edited, 1), KC_A);
        assert_eq!(edited.keymap, mock.read().keymap);
    }

    #[test]
    fn a_change_can_be_undone_and_the_first_backup_restored() {
        let mut mock = Mock::new();
        let edited = mock.set(0, KC_A);
        assert_eq!((edited.undo_steps, edited.can_restore_backup), (1, true));
    }

    #[test]
    fn reading_before_any_change_offers_no_undo_and_no_backup() {
        let keymap = Mock::new().read();
        assert_eq!((keymap.undo_steps, keymap.can_restore_backup), (0, false));
    }

    #[test]
    fn undo_puts_back_the_key_from_before_the_newest_change() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.set(0, KC_B);
        let undone = mock.undo();
        assert_eq!((keycode_at(&undone, 0), undone.undo_steps), (KC_A, 1));
    }

    #[test]
    fn undoing_every_change_leaves_the_keymap_as_it_was_and_nothing_to_undo() {
        let mut mock = Mock::new();
        let before = mock.read().keymap;
        mock.set(0, KC_A);
        mock.set(1, KC_B);
        mock.undo();
        let undone = mock.undo();
        assert_eq!((undone.keymap, undone.undo_steps), (before, 0));
    }

    #[test]
    fn refuses_to_undo_before_any_change() {
        let mut mock = Mock::new();
        let error = undo(mock.keyboard.connect(), &mock.device, &mut mock.history).unwrap_err();
        assert_eq!(
            error.to_string(),
            "nothing to undo: the app has not changed this keyboard's keymap since it started"
        );
    }

    #[test]
    fn restore_backup_puts_back_the_keymap_from_before_the_first_change() {
        let mut mock = Mock::new();
        // A non-zero key, so a restore that wrote the zero seed back would not pass.
        mock.set(0, KC_B);
        mock.history = KeymapHistory::default();
        let first = mock.read().keymap;
        mock.set(0, KC_A);
        mock.set(1, KC_C);
        assert_eq!(mock.restore_backup().keymap, first);
    }

    #[test]
    fn undo_takes_a_restore_back() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.set(0, KC_B);
        mock.restore_backup();
        let undone = mock.undo();
        assert_eq!((keycode_at(&undone, 0), undone.undo_steps), (KC_B, 2));
    }

    #[test]
    fn refuses_to_restore_before_any_change() {
        let mut mock = Mock::new();
        let error =
            restore_backup(mock.keyboard.connect(), &mock.device, &mut mock.history).unwrap_err();
        assert_eq!(
            error.to_string(),
            "no keymap backup yet: the app keeps one when it first changes this keyboard's keymap"
        );
    }

    #[test]
    fn refuses_a_matrix_slot_without_a_physical_key() {
        let mut mock = Mock::new();
        let error = set_keycode(
            mock.keyboard.connect(),
            &mock.device,
            &mut mock.history,
            KeyAssignment {
                layer: 0,
                row: 0,
                column: 1,
                keycode: KC_A,
            },
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "no physical key at (0, 1) in the supported V5 Max ISO layout"
        );
    }

    #[test]
    fn refuses_a_layer_the_keyboard_does_not_have_and_keeps_no_backup() {
        let mut mock = Mock::new();
        let error = set_keycode(
            mock.keyboard.connect(),
            &mock.device,
            &mut mock.history,
            assignment(4, KC_A),
        )
        .unwrap_err();
        assert_eq!(
            (error.to_string(), mock.read().undo_steps),
            ("layer 4 is unavailable; select 0..=3".to_owned(), 0)
        );
    }

    /// The VIA request that sets one keycode: `id_dynamic_keymap_set_keycode = 0x05` in QMK's
    /// `quantum/via.h` (see `openkeychron-via`).
    const SET_KEYCODE_REQUEST: u8 = 0x05;

    /// A mock keyboard connection whose keycode writes fail, as an unplugged cable would.
    struct FailingKeycodeWrites(MockTransport);

    impl HidTransport for FailingKeycodeWrites {
        fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
            if report.first() == Some(&SET_KEYCODE_REQUEST) {
                return Err(TransportError::Device("unplugged".to_owned()));
            }
            self.0.write(report)
        }

        fn read(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError> {
            self.0.read(timeout)
        }
    }

    #[test]
    fn a_write_that_fails_after_the_backup_says_undo_puts_it_back() {
        let mut mock = Mock::new();
        let error = set_keycode(
            FailingKeycodeWrites(mock.keyboard.connect()),
            &mock.device,
            &mut mock.history,
            assignment(0, KC_A),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "device I/O failed: unplugged; the keyboard may have changed: Undo puts back the \
             keymap from before this change"
        );
    }

    #[test]
    fn a_write_that_fails_after_the_backup_leaves_it_to_undo() {
        let mut mock = Mock::new();
        let before = mock.read().keymap;
        set_keycode(
            FailingKeycodeWrites(mock.keyboard.connect()),
            &mock.device,
            &mut mock.history,
            assignment(0, KC_A),
        )
        .unwrap_err();
        assert_eq!(mock.undo().keymap, before);
    }

    #[test]
    fn refuses_every_write_through_a_receiver_before_sending_anything() {
        type Write = for<'a> fn(
            Lent<'a>,
            &KeychronDevice,
            &mut KeymapHistory,
        ) -> Result<EditableKeymap, CommandError>;
        let writes: [Write; 3] = [
            |transport, device, history| set_keycode(transport, device, history, assignment(0, 4)),
            |transport, device, history| undo(transport, device, history),
            |transport, device, history| restore_backup(transport, device, history),
        ];
        for write in writes {
            let mut replay = replay("cli-info.toml");
            let error = write(
                lend(&mut replay.transport),
                &replay.device,
                &mut KeymapHistory::default(),
            )
            .unwrap_err();
            assert_eq!(
                error.to_string(),
                "keymap writes require the V5 Max ISO (3434:0951) on a USB cable; receiver and \
                 unverified-model writes are refused (got receiver 3434:d030)"
            );
            assert_eq!(replay.finish(), load_fixture("cli-info.toml").events);
        }
    }
}
