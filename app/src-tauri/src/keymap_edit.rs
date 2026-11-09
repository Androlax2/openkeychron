//! The keymap writes the UI makes: set one key or what one turn of an encoder sends, undo the last
//! change, redo what an undo took back, and restore the keymap from before the first change. Each goes through
//! `openkeychron_guard::keymap`, which refuses a receiver or an unverified model, has this module
//! keep the keymap it replaces in memory before it sends anything, and reads the whole keymap
//! back. This module only decides which backup to
//! keep and which one to write back.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use openkeychron_guard::keymap::{self as guarded_keymap, EncoderTurn, KeyPosition, KeymapBackup};
use openkeychron_guard::{GuardError, GuardedWriteError, require_write_verified};
use openkeychron_hid::{HidTransport, KeychronDevice};
use openkeychron_via::{EncoderDirection, Via};
use serde::{Deserialize, Serialize};

use crate::error::CommandError;
use crate::keymap::{self, Keymap};
use crate::source::Connection;

/// What the write gate's refusal names.
const WRITES: &str = "keymap writes";

/// A keymap, and what the app can undo and redo on its device.
#[derive(Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EditableKeymap {
    pub keymap: Keymap,
    /// How many changes Undo can take back, one at a time, newest first.
    pub undo_steps: usize,
    /// How many changes Redo can put back, one at a time, the last one undone first.
    pub redo_steps: usize,
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

/// What one turn of an encoder sends on `layer`: the encoder's index, as `Key.encoder` names it,
/// the direction, and the keycode it gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EncoderAssignment {
    pub layer: u8,
    pub encoder: u8,
    pub direction: TurnDirection,
    pub keycode: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum TurnDirection {
    Clockwise,
    CounterClockwise,
}

impl From<TurnDirection> for EncoderDirection {
    fn from(direction: TurnDirection) -> Self {
        match direction {
            TurnDirection::Clockwise => Self::Clockwise,
            TurnDirection::CounterClockwise => Self::CounterClockwise,
        }
    }
}

/// Every keymap the app replaced on one device, kept in memory until the app quits or another
/// plug-in takes the device's path.
#[derive(Debug, Default)]
pub struct KeymapHistory {
    /// The keymap from before the app's first change: what Restore backup writes back.
    first_backup: Option<KeymapBackup>,
    /// The keymap from before each change Undo can take back, oldest first. A restore's backup
    /// goes here too, so Undo takes a restore back.
    undo: Vec<KeymapBackup>,
    /// The keymap each undo replaced, oldest first: what Redo writes back. A redo's backup goes
    /// to `undo`, so Undo takes a redo back. A new change or a restore empties it.
    undone: Vec<KeymapBackup>,
    /// The keymap the app's last write left the keyboard with, as its read-back verified. `None`
    /// before any write, and after a write that failed part-way left the keymap unknown: Undo
    /// and Redo then write without comparing, since writing the keymap back is how to recover.
    left_as: Option<KeymapBackup>,
}

impl KeymapHistory {
    /// Notes what a guarded write left the keyboard with: `expected`, given the keymap the write
    /// replaced, once it read back; unknown once it failed after its backup.
    fn note<E: Into<CommandError>>(
        &mut self,
        outcome: Result<KeymapBackup, GuardedWriteError<KeymapBackup, E>>,
        expected: impl FnOnce(&KeymapBackup) -> KeymapBackup,
        recovery: &'static str,
    ) -> Result<(), CommandError> {
        match outcome {
            Ok(replaced) => {
                self.left_as = Some(expected(&replaced));
                Ok(())
            }
            Err(GuardedWriteError::NothingWritten(error)) => Err(error.into()),
            Err(GuardedWriteError::AfterBackup { error, .. }) => {
                self.left_as = None;
                Err(CommandError::WriteFailedAfterBackup {
                    error: Box::new(error.into()),
                    recovery,
                })
            }
        }
    }

    /// Refuses `write` when the keyboard holds a keymap other than the one the app's last write
    /// left: writing over it would also take back changes made with another program.
    fn require_left_as_the_app_left_it(
        &self,
        replaced: &KeymapBackup,
        write: &'static str,
    ) -> Result<(), CommandError> {
        if self
            .left_as
            .as_ref()
            .is_some_and(|left_as| left_as != replaced)
        {
            return Err(CommandError::KeymapChangedOutsideApp { write });
        }
        Ok(())
    }

    fn with(&self, keymap: Keymap) -> EditableKeymap {
        EditableKeymap {
            keymap,
            undo_steps: self.undo.len(),
            redo_steps: self.undone.len(),
            can_restore_backup: self.first_backup.is_some(),
        }
    }
}

/// The keymap history of each device, by its connection: a history taken from one keyboard is
/// never offered to another plugged in on the same path, where Restore backup would write the
/// first keyboard's keymap onto it.
#[derive(Debug, Default)]
pub struct KeymapHistories(Mutex<HistoriesByConnection>);

#[derive(Debug, Default)]
pub struct HistoriesByConnection(HashMap<Connection, KeymapHistory>);

impl HistoriesByConnection {
    /// The history of `connection`, empty for a new one. Drops the history of any earlier
    /// connection on the same path: that plug-in is gone, and the kernel never names another
    /// one the same.
    pub fn of(&mut self, connection: Connection) -> &mut KeymapHistory {
        self.0
            .retain(|kept, _| kept.path != connection.path || *kept == connection);
        self.0.entry(connection).or_default()
    }
}

impl KeymapHistories {
    pub fn lock(&self) -> MutexGuard<'_, HistoriesByConnection> {
        // A history changes only through whole-value pushes, pops and assignments, so a command
        // that panicked while holding the lock left every backup in it complete.
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
    let outcome = guarded_keymap::set_keycode::<_, GuardError>(
        &mut via,
        &verified,
        assignment.layer,
        key,
        assignment.keycode,
        |previous| {
            keep_before_a_change(history, previous);
            Ok(())
        },
    );
    history.note(
        outcome,
        |previous| with_assignment(previous, assignment),
        "Undo puts back the keymap from before this change",
    )?;
    read(via.into_transport(), device, history)
}

/// Sets what one turn of an encoder sends, then returns the keymap read again after the write.
pub fn set_encoder_keycode<T: HidTransport>(
    transport: T,
    device: &KeychronDevice,
    history: &mut KeymapHistory,
    assignment: EncoderAssignment,
) -> Result<EditableKeymap, CommandError> {
    let verified = require_write_verified(WRITES, device)?;
    let turn = EncoderTurn::new(assignment.encoder, assignment.direction.into())?;
    let mut via = Via::new(transport);
    let outcome = guarded_keymap::set_encoder_keycode::<_, GuardError>(
        &mut via,
        &verified,
        assignment.layer,
        turn,
        assignment.keycode,
        |previous| {
            keep_before_a_change(history, previous);
            Ok(())
        },
    );
    history.note(
        outcome,
        |previous| with_encoder_assignment(previous, assignment),
        "Undo puts back the keymap from before this change",
    )?;
    read(via.into_transport(), device, history)
}

/// Writes back the keymap from before the newest change Undo can take back. Refused, before
/// anything is sent, when the keymap is not as the app's last write left it: changes made with
/// another program since then would be taken back too.
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
    let outcome =
        guarded_keymap::restore::<_, CommandError>(&mut via, &verified, &previous, |replaced| {
            history.require_left_as_the_app_left_it(replaced, "Undo")?;
            keep_as_a_step(&mut history.undone, replaced, &previous);
            Ok(())
        });
    history.note(outcome, |_| previous.clone(), "Undo again to retry")?;
    history.undo.pop();
    read(via.into_transport(), device, history)
}

/// Writes back the keymap the newest undo replaced. Refused, before anything is sent, when the
/// keymap is not as the app's last write left it, as `undo` is.
pub fn redo<T: HidTransport>(
    transport: T,
    device: &KeychronDevice,
    history: &mut KeymapHistory,
) -> Result<EditableKeymap, CommandError> {
    let verified = require_write_verified(WRITES, device)?;
    let undone = history
        .undone
        .last()
        .cloned()
        .ok_or(CommandError::NothingToRedo)?;
    let mut via = Via::new(transport);
    let outcome =
        guarded_keymap::restore::<_, CommandError>(&mut via, &verified, &undone, |replaced| {
            history.require_left_as_the_app_left_it(replaced, "Redo")?;
            keep_as_a_step(&mut history.undo, replaced, &undone);
            Ok(())
        });
    history.note(outcome, |_| undone.clone(), "Redo again to retry")?;
    history.undone.pop();
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
    let outcome =
        guarded_keymap::restore::<_, GuardError>(&mut via, &verified, &backup, |replaced| {
            history.undo.push(replaced.clone());
            history.undone.clear();
            Ok(())
        });
    history.note(
        outcome,
        |_| backup.clone(),
        "Undo puts back the keymap from before the restore, and Restore backup tries again",
    )?;
    read(via.into_transport(), device, history)
}

/// Keeps the keymap a new change replaces: the first one is what Restore backup writes back, and
/// each is a step Undo takes back.
fn keep_before_a_change(history: &mut KeymapHistory, previous: &KeymapBackup) {
    history.first_backup.get_or_insert_with(|| previous.clone());
    history.undo.push(previous.clone());
    history.undone.clear();
}

/// Keeps the keymap an undo or a redo replaces as a step the other one can write back, unless the
/// step would put back nothing: the keyboard already holds `wanted`, or the newest step already
/// holds `replaced`. Both happen when a write failed before it changed anything and is tried
/// again, or the other button is pressed after it; each would otherwise add a step that writes
/// the keymap the keyboard has.
fn keep_as_a_step(steps: &mut Vec<KeymapBackup>, replaced: &KeymapBackup, wanted: &KeymapBackup) {
    if replaced != wanted && steps.last() != Some(replaced) {
        steps.push(replaced.clone());
    }
}

/// The keymap `guarded_keymap::set_keycode` requires the keyboard to read back: `previous`
/// with the one key changed. The guard checked the layer and the key before writing.
fn with_assignment(previous: &KeymapBackup, assignment: KeyAssignment) -> KeymapBackup {
    let mut expected = previous.clone();
    let slot = (usize::from(assignment.layer) * usize::from(expected.rows)
        + usize::from(assignment.row))
        * usize::from(expected.columns)
        + usize::from(assignment.column);
    expected.matrix[slot] = assignment.keycode;
    expected
}

/// The keymap `guarded_keymap::set_encoder_keycode` requires the keyboard to read back:
/// `previous` with the one turn changed. The guard checked the layer and the encoder before
/// writing, and the supported layout's only encoder is the one a backup holds per layer.
fn with_encoder_assignment(previous: &KeymapBackup, assignment: EncoderAssignment) -> KeymapBackup {
    let mut expected = previous.clone();
    let pair = &mut expected.encoders[usize::from(assignment.layer)];
    match assignment.direction {
        TurnDirection::Clockwise => pair.clockwise = assignment.keycode,
        TurnDirection::CounterClockwise => pair.counter_clockwise = assignment.keycode,
    }
    expected
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use openkeychron_hid::{HidTransport, KeychronDevice, TransportError, fixture_device};
    use openkeychron_mock::{MockKeyboard, MockTransport};

    use super::{
        EditableKeymap, EncoderAssignment, KeyAssignment, KeymapHistories, KeymapHistory,
        TurnDirection, read, redo, restore_backup, set_encoder_keycode, set_keycode, undo,
    };
    use crate::error::CommandError;
    use crate::source::Connection;
    use crate::test_support::{Lent, lend, load_fixture, replay, synthetic_path};

    /// Row 2, column 1: the key labelled `A` on the FR ISO layout.
    const ROW: u8 = 2;
    const COLUMN: u8 = 1;
    const COLUMNS: usize = 19;
    const KC_A: u16 = 0x0004;
    const KC_B: u16 = 0x0005;
    const KC_C: u16 = 0x0006;
    const KC_VOLU: u16 = 0x00a9;
    const KC_VOLD: u16 = 0x00aa;

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

        fn turn(&mut self, layer: u8, direction: TurnDirection, keycode: u16) -> EditableKeymap {
            set_encoder_keycode(
                self.keyboard.connect(),
                &self.device,
                &mut self.history,
                turn_assignment(layer, direction, keycode),
            )
            .unwrap()
        }

        fn undo(&mut self) -> EditableKeymap {
            undo(self.keyboard.connect(), &self.device, &mut self.history).unwrap()
        }

        fn redo(&mut self) -> EditableKeymap {
            redo(self.keyboard.connect(), &self.device, &mut self.history).unwrap()
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

    fn turn_assignment(layer: u8, direction: TurnDirection, keycode: u16) -> EncoderAssignment {
        EncoderAssignment {
            layer,
            encoder: 0,
            direction,
            keycode,
        }
    }

    /// What the knob sends turned counter-clockwise and clockwise on `layer`.
    fn knob_at(keymap: &EditableKeymap, layer: usize) -> (u16, u16) {
        let knob = &keymap.keymap.layers[layer].encoders[0];
        (knob.counter_clockwise.keycode, knob.clockwise.keycode)
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
    fn a_change_offers_one_undo_step_and_the_backup() {
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
            "nothing left to undo: Undo takes back only the keymap changes the app made since it \
             started"
        );
    }

    #[test]
    fn an_undo_offers_one_redo_step() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        assert_eq!(mock.undo().redo_steps, 1);
    }

    #[test]
    fn reading_before_any_undo_offers_no_redo() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        assert_eq!(mock.read().redo_steps, 0);
    }

    #[test]
    fn redo_puts_back_the_change_the_last_undo_took_back() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.set(0, KC_B);
        mock.undo();
        let redone = mock.redo();
        assert_eq!(
            (keycode_at(&redone, 0), redone.undo_steps, redone.redo_steps),
            (KC_B, 2, 0)
        );
    }

    #[test]
    fn redoing_every_undone_change_leaves_the_keymap_as_after_the_last_change() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        let after = mock.set(1, KC_B).keymap;
        mock.undo();
        mock.undo();
        mock.redo();
        let redone = mock.redo();
        assert_eq!((redone.keymap, redone.redo_steps), (after, 0));
    }

    #[test]
    fn undo_takes_a_redo_back() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.undo();
        mock.redo();
        let undone = mock.undo();
        assert_eq!(
            (keycode_at(&undone, 0), undone.undo_steps, undone.redo_steps),
            (0, 0, 1)
        );
    }

    #[test]
    fn a_new_change_after_an_undo_drops_what_redo_could_put_back() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.undo();
        assert_eq!(mock.set(0, KC_B).redo_steps, 0);
    }

    #[test]
    fn restoring_the_backup_after_an_undo_drops_what_redo_could_put_back() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.set(0, KC_B);
        mock.undo();
        assert_eq!(mock.restore_backup().redo_steps, 0);
    }

    #[test]
    fn a_refused_write_keeps_what_redo_can_put_back() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.undo();
        set_on_layer_four(&mut mock);
        assert_eq!(mock.read().redo_steps, 1);
    }

    #[test]
    fn refuses_to_redo_before_any_undo() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        let error = redo(mock.keyboard.connect(), &mock.device, &mut mock.history).unwrap_err();
        assert_eq!(
            error.to_string(),
            "nothing left to redo: Redo puts back only what Undo took back since the app's last \
             change"
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

    fn set_on_layer_four(mock: &mut Mock) -> CommandError {
        set_keycode(
            mock.keyboard.connect(),
            &mock.device,
            &mut mock.history,
            assignment(4, KC_A),
        )
        .unwrap_err()
    }

    #[test]
    fn refuses_a_layer_the_keyboard_does_not_have() {
        let error = set_on_layer_four(&mut Mock::new());
        assert_eq!(error.to_string(), "layer 4 is unavailable; select 0..=3");
    }

    #[test]
    fn a_refused_write_offers_no_undo() {
        let mut mock = Mock::new();
        set_on_layer_four(&mut mock);
        assert_eq!(mock.read().undo_steps, 0);
    }

    /// Sets the key as another program would: on the same keyboard, outside the app's history.
    fn set_outside_the_app(mock: &Mock, keycode: u16) {
        set_keycode(
            mock.keyboard.connect(),
            &mock.device,
            &mut KeymapHistory::default(),
            assignment(0, keycode),
        )
        .unwrap();
    }

    fn undo_after_a_change_outside_the_app(mock: &mut Mock) -> CommandError {
        mock.set(0, KC_A);
        set_outside_the_app(mock, KC_C);
        undo(mock.keyboard.connect(), &mock.device, &mut mock.history).unwrap_err()
    }

    #[test]
    fn undo_refuses_to_write_over_a_keymap_changed_outside_the_app() {
        let error = undo_after_a_change_outside_the_app(&mut Mock::new());
        assert_eq!(
            error.to_string(),
            "the keymap is not as the app left it, so Undo would also take back changes made \
             outside the app; nothing was written. Restore backup still puts back the keymap \
             from before the app's first change"
        );
    }

    fn redo_after_a_change_outside_the_app(mock: &mut Mock) -> CommandError {
        mock.set(0, KC_A);
        mock.undo();
        set_outside_the_app(mock, KC_C);
        redo(mock.keyboard.connect(), &mock.device, &mut mock.history).unwrap_err()
    }

    #[test]
    fn redo_refuses_to_write_over_a_keymap_changed_outside_the_app() {
        let error = redo_after_a_change_outside_the_app(&mut Mock::new());
        assert_eq!(
            error.to_string(),
            "the keymap is not as the app left it, so Redo would also take back changes made \
             outside the app; nothing was written. Restore backup still puts back the keymap \
             from before the app's first change"
        );
    }

    #[test]
    fn a_refused_redo_leaves_the_keyboard_and_the_redo_step_as_they_were() {
        let mut mock = Mock::new();
        redo_after_a_change_outside_the_app(&mut mock);
        let keymap = mock.read();
        assert_eq!(
            (keycode_at(&keymap, 0), keymap.undo_steps, keymap.redo_steps),
            (KC_C, 0, 1)
        );
    }

    #[test]
    fn a_refused_undo_leaves_the_keyboard_and_the_undo_step_as_they_were() {
        let mut mock = Mock::new();
        undo_after_a_change_outside_the_app(&mut mock);
        let keymap = mock.read();
        assert_eq!(
            (keycode_at(&keymap, 0), keymap.undo_steps, keymap.redo_steps),
            (KC_C, 1, 0)
        );
    }

    /// QMK's `quantum/via.h` request IDs, as `openkeychron-via` sends them:
    /// `id_dynamic_keymap_set_keycode = 0x05` sets one key, and
    /// `id_dynamic_keymap_set_buffer = 0x13` writes the keymap a restore sends.
    const SET_KEYCODE_REQUEST: u8 = 0x05;
    const SET_KEYMAP_BUFFER_REQUEST: u8 = 0x13;

    /// A mock keyboard connection on which one VIA request fails, as an unplugged cable would.
    struct FailingRequest {
        request: u8,
        transport: MockTransport,
    }

    fn failing_keycode_writes(mock: &Mock) -> FailingRequest {
        FailingRequest {
            request: SET_KEYCODE_REQUEST,
            transport: mock.keyboard.connect(),
        }
    }

    impl HidTransport for FailingRequest {
        fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
            if report.first() == Some(&self.request) {
                return Err(TransportError::Device("unplugged".to_owned()));
            }
            self.transport.write(report)
        }

        fn read(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError> {
            self.transport.read(timeout)
        }
    }

    fn failing_keymap_buffer(mock: &Mock) -> FailingRequest {
        FailingRequest {
            request: SET_KEYMAP_BUFFER_REQUEST,
            transport: mock.keyboard.connect(),
        }
    }

    #[test]
    fn an_undo_that_fails_after_its_backup_says_to_undo_again() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        let error = undo(
            failing_keymap_buffer(&mock),
            &mock.device,
            &mut mock.history,
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "device I/O failed: unplugged; the keyboard may have changed: Undo again to retry \
             while the keyboard stays plugged in; the app starts a new history once it is \
             plugged in again"
        );
    }

    #[test]
    fn undoing_again_after_a_failed_undo_puts_the_key_back() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        undo(
            failing_keymap_buffer(&mock),
            &mock.device,
            &mut mock.history,
        )
        .unwrap_err();
        let undone = mock.undo();
        assert_eq!((keycode_at(&undone, 0), undone.undo_steps), (0, 0));
    }

    #[test]
    fn undoing_again_after_a_failed_undo_offers_one_redo_step() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        undo(
            failing_keymap_buffer(&mock),
            &mock.device,
            &mut mock.history,
        )
        .unwrap_err();
        assert_eq!(mock.undo().redo_steps, 1);
    }

    #[test]
    fn redo_after_a_failed_undo_offers_one_undo_step() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        undo(
            failing_keymap_buffer(&mock),
            &mock.device,
            &mut mock.history,
        )
        .unwrap_err();
        let redone = mock.redo();
        assert_eq!(
            (keycode_at(&redone, 0), redone.undo_steps, redone.redo_steps),
            (KC_A, 1, 0)
        );
    }

    #[test]
    fn a_redo_that_fails_after_its_backup_says_to_redo_again() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.undo();
        let error = redo(
            failing_keymap_buffer(&mock),
            &mock.device,
            &mut mock.history,
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "device I/O failed: unplugged; the keyboard may have changed: Redo again to retry \
             while the keyboard stays plugged in; the app starts a new history once it is \
             plugged in again"
        );
    }

    #[test]
    fn redoing_again_after_a_failed_redo_puts_the_change_back() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.undo();
        redo(
            failing_keymap_buffer(&mock),
            &mock.device,
            &mut mock.history,
        )
        .unwrap_err();
        let redone = mock.redo();
        assert_eq!((keycode_at(&redone, 0), redone.redo_steps), (KC_A, 0));
    }

    #[test]
    fn redoing_again_after_a_failed_redo_offers_one_undo_step() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.undo();
        redo(
            failing_keymap_buffer(&mock),
            &mock.device,
            &mut mock.history,
        )
        .unwrap_err();
        assert_eq!(mock.redo().undo_steps, 1);
    }

    #[test]
    fn undo_after_a_failed_redo_offers_the_redo_again() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.undo();
        redo(
            failing_keymap_buffer(&mock),
            &mock.device,
            &mut mock.history,
        )
        .unwrap_err();
        let undone = mock.undo();
        assert_eq!(
            (keycode_at(&undone, 0), undone.undo_steps, undone.redo_steps),
            (0, 0, 1)
        );
    }

    #[test]
    fn a_restore_that_fails_after_its_backup_says_how_to_get_back() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        let error = restore_backup(
            failing_keymap_buffer(&mock),
            &mock.device,
            &mut mock.history,
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "device I/O failed: unplugged; the keyboard may have changed: Undo puts back the \
             keymap from before the restore, and Restore backup tries again while the keyboard \
             stays plugged in; the app starts a new history once it is plugged in again"
        );
    }

    #[test]
    fn a_write_that_fails_after_the_backup_says_undo_puts_it_back() {
        let mut mock = Mock::new();
        let error = set_keycode(
            failing_keycode_writes(&mock),
            &mock.device,
            &mut mock.history,
            assignment(0, KC_A),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "device I/O failed: unplugged; the keyboard may have changed: Undo puts back the \
             keymap from before this change while the keyboard stays plugged in; the app starts \
             a new history once it is plugged in again"
        );
    }

    #[test]
    fn a_write_that_fails_after_the_backup_leaves_it_to_undo() {
        let mut mock = Mock::new();
        let before = mock.read().keymap;
        set_keycode(
            failing_keycode_writes(&mock),
            &mock.device,
            &mut mock.history,
            assignment(0, KC_A),
        )
        .unwrap_err();
        assert_eq!(mock.undo().keymap, before);
    }

    #[test]
    fn returns_the_keymap_read_back_after_setting_a_knob_turn() {
        let mut mock = Mock::new();
        let edited = mock.turn(1, TurnDirection::Clockwise, KC_VOLU);
        assert_eq!(knob_at(&edited, 1), (0, KC_VOLU));
        assert_eq!(edited.keymap, mock.read().keymap);
    }

    #[test]
    fn sets_the_counter_clockwise_turn_alone() {
        let mut mock = Mock::new();
        let edited = mock.turn(0, TurnDirection::CounterClockwise, KC_VOLD);
        assert_eq!(knob_at(&edited, 0), (KC_VOLD, 0));
    }

    #[test]
    fn a_knob_change_offers_one_undo_step_and_the_backup() {
        let mut mock = Mock::new();
        let edited = mock.turn(0, TurnDirection::Clockwise, KC_VOLU);
        assert_eq!((edited.undo_steps, edited.can_restore_backup), (1, true));
    }

    #[test]
    fn undo_puts_back_the_knob_turn_from_before_the_change() {
        let mut mock = Mock::new();
        mock.turn(0, TurnDirection::Clockwise, KC_VOLU);
        mock.turn(0, TurnDirection::Clockwise, KC_VOLD);
        let undone = mock.undo();
        assert_eq!((knob_at(&undone, 0), undone.undo_steps), ((0, KC_VOLU), 1));
    }

    #[test]
    fn undoing_a_knob_change_leaves_an_earlier_key_change() {
        let mut mock = Mock::new();
        mock.set(0, KC_A);
        mock.turn(0, TurnDirection::Clockwise, KC_VOLU);
        let undone = mock.undo();
        assert_eq!(
            (keycode_at(&undone, 0), knob_at(&undone, 0)),
            (KC_A, (0, 0))
        );
    }

    #[test]
    fn redo_puts_back_the_knob_turn_the_last_undo_took_back() {
        let mut mock = Mock::new();
        mock.turn(2, TurnDirection::CounterClockwise, KC_VOLD);
        mock.undo();
        let redone = mock.redo();
        assert_eq!(
            (knob_at(&redone, 2), redone.undo_steps, redone.redo_steps),
            ((KC_VOLD, 0), 1, 0)
        );
    }

    #[test]
    fn restore_backup_puts_back_the_knob_from_before_the_first_change() {
        let mut mock = Mock::new();
        // A non-zero turn, so a restore that wrote the zero seed back would not pass.
        mock.turn(0, TurnDirection::Clockwise, KC_VOLD);
        mock.history = KeymapHistory::default();
        let first = mock.read().keymap;
        mock.turn(0, TurnDirection::Clockwise, KC_VOLU);
        mock.set(1, KC_C);
        assert_eq!(mock.restore_backup().keymap, first);
    }

    #[test]
    fn refuses_an_encoder_the_layout_does_not_have() {
        let mut mock = Mock::new();
        let error = set_encoder_keycode(
            mock.keyboard.connect(),
            &mock.device,
            &mut mock.history,
            EncoderAssignment {
                encoder: 1,
                ..turn_assignment(0, TurnDirection::Clockwise, KC_VOLU)
            },
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "no encoder 1 in the supported V5 Max ISO layout"
        );
    }

    #[test]
    fn refuses_a_knob_turn_on_a_layer_the_keyboard_does_not_have() {
        let mut mock = Mock::new();
        let error = set_encoder_keycode(
            mock.keyboard.connect(),
            &mock.device,
            &mut mock.history,
            turn_assignment(4, TurnDirection::Clockwise, KC_VOLU),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "layer 4 is unavailable; select 0..=3");
    }

    /// QMK's `quantum/via.h`: `id_dynamic_keymap_set_encoder = 0x15`.
    const SET_ENCODER_REQUEST: u8 = 0x15;

    fn set_knob_while_unplugged(mock: &mut Mock) -> CommandError {
        set_encoder_keycode(
            FailingRequest {
                request: SET_ENCODER_REQUEST,
                transport: mock.keyboard.connect(),
            },
            &mock.device,
            &mut mock.history,
            turn_assignment(0, TurnDirection::Clockwise, KC_VOLU),
        )
        .unwrap_err()
    }

    #[test]
    fn a_knob_write_that_fails_after_the_backup_says_undo_puts_it_back() {
        let error = set_knob_while_unplugged(&mut Mock::new());
        assert_eq!(
            error.to_string(),
            "device I/O failed: unplugged; the keyboard may have changed: Undo puts back the \
             keymap from before this change while the keyboard stays plugged in; the app starts \
             a new history once it is plugged in again"
        );
    }

    #[test]
    fn a_knob_write_that_fails_after_the_backup_leaves_it_to_undo() {
        let mut mock = Mock::new();
        let before = mock.read().keymap;
        set_knob_while_unplugged(&mut mock);
        assert_eq!(mock.undo().keymap, before);
    }

    const PATH: &str = "/dev/hidraw5";
    const FIRST_PLUG_IN: &str = "0003:3434:0951.0007";
    const NEXT_PLUG_IN: &str = "0003:3434:0951.0008";

    fn connection(path: &str, hid_device: &str) -> Connection {
        Connection {
            path: path.to_owned(),
            hid_device: Some(hid_device.to_owned()),
        }
    }

    /// Sets a key on `mock`'s keyboard through the history `histories` keeps for `connection`.
    fn set_through(histories: &KeymapHistories, mock: &Mock, connection: Connection) {
        set_keycode(
            mock.keyboard.connect(),
            &mock.device,
            histories.lock().of(connection),
            assignment(0, KC_A),
        )
        .unwrap();
    }

    fn read_through(
        histories: &KeymapHistories,
        mock: &Mock,
        connection: Connection,
    ) -> EditableKeymap {
        read(
            mock.keyboard.connect(),
            &mock.device,
            histories.lock().of(connection),
        )
        .unwrap()
    }

    #[test]
    fn a_keyboard_keeps_its_history_while_it_stays_plugged_in() {
        let histories = KeymapHistories::default();
        let mock = Mock::new();
        set_through(&histories, &mock, connection(PATH, FIRST_PLUG_IN));
        let keymap = read_through(&histories, &mock, connection(PATH, FIRST_PLUG_IN));
        assert_eq!((keymap.undo_steps, keymap.can_restore_backup), (1, true));
    }

    #[test]
    fn a_keyboard_plugged_in_again_on_the_same_path_starts_without_history() {
        let histories = KeymapHistories::default();
        let mock = Mock::new();
        set_through(&histories, &mock, connection(PATH, FIRST_PLUG_IN));
        let keymap = read_through(&histories, &mock, connection(PATH, NEXT_PLUG_IN));
        assert_eq!((keymap.undo_steps, keymap.can_restore_backup), (0, false));
    }

    #[test]
    fn restore_backup_refuses_another_keyboard_plugged_in_on_the_same_path() {
        let histories = KeymapHistories::default();
        set_through(&histories, &Mock::new(), connection(PATH, FIRST_PLUG_IN));
        let other = Mock::new();
        let error = restore_backup(
            other.keyboard.connect(),
            &other.device,
            histories.lock().of(connection(PATH, NEXT_PLUG_IN)),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "no keymap backup yet: the app keeps one when it first changes this keyboard's keymap"
        );
    }

    #[test]
    fn forgets_a_history_once_another_plug_in_takes_its_path() {
        let histories = KeymapHistories::default();
        let mock = Mock::new();
        set_through(&histories, &mock, connection(PATH, FIRST_PLUG_IN));
        read_through(&histories, &mock, connection(PATH, NEXT_PLUG_IN));
        let keymap = read_through(&histories, &mock, connection(PATH, FIRST_PLUG_IN));
        assert_eq!(keymap.undo_steps, 0);
    }

    #[test]
    fn a_keyboard_on_another_path_keeps_its_history() {
        let histories = KeymapHistories::default();
        let mock = Mock::new();
        set_through(&histories, &mock, connection(PATH, FIRST_PLUG_IN));
        read_through(&histories, &mock, connection("/dev/hidraw6", NEXT_PLUG_IN));
        let keymap = read_through(&histories, &mock, connection(PATH, FIRST_PLUG_IN));
        assert_eq!(keymap.undo_steps, 1);
    }

    #[test]
    fn refuses_every_write_through_a_receiver_before_sending_anything() {
        type Write = for<'a> fn(
            Lent<'a>,
            &KeychronDevice,
            &mut KeymapHistory,
        ) -> Result<EditableKeymap, CommandError>;
        let writes: [Write; 5] = [
            |transport, device, history| set_keycode(transport, device, history, assignment(0, 4)),
            |transport, device, history| {
                set_encoder_keycode(
                    transport,
                    device,
                    history,
                    turn_assignment(0, TurnDirection::Clockwise, 4),
                )
            },
            |transport, device, history| undo(transport, device, history),
            |transport, device, history| redo(transport, device, history),
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
