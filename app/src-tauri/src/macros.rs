//! `macros`: every macro slot, decoded into actions where the bytes allow it. Only reads.

use openkeychron_hid::{HidTransport, KeychronDevice};
use openkeychron_via::{MacroAction, Via, decode_macro, slots};
use serde::Serialize;

use crate::error::CommandError;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MacroList {
    pub macro_count: u8,
    pub buffer_size: u16,
    /// False when the buffer's last byte is not zero: the firmware then plays no macro.
    pub macros_enabled: bool,
    pub macros: Vec<MacroSlot>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MacroSlot {
    pub slot: u8,
    pub bytes: Vec<u8>,
    pub content: MacroContent,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MacroContent {
    Actions {
        actions: Vec<Action>,
    },
    /// Bytes this build cannot read as a macro; `error` says where.
    Unreadable {
        error: String,
    },
}

/// Keycodes are 8-bit basic keycodes. `Text` is typed by the firmware as US-ANSI keycodes.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Action {
    Text(String),
    Tap(u8),
    Down(u8),
    Up(u8),
    Delay(u16),
}

impl From<MacroAction> for Action {
    fn from(action: MacroAction) -> Self {
        match action {
            MacroAction::Text(text) => Self::Text(text),
            MacroAction::Tap(key) => Self::Tap(key),
            MacroAction::Down(key) => Self::Down(key),
            MacroAction::Up(key) => Self::Up(key),
            MacroAction::Delay(milliseconds) => Self::Delay(milliseconds),
        }
    }
}

pub fn read<T: HidTransport>(
    transport: T,
    device: &KeychronDevice,
) -> Result<MacroList, CommandError> {
    let mut via = Via::new(transport);
    let via_error = |error| CommandError::via(&error, device.is_receiver);
    let macro_count = via.macro_count().map_err(via_error)?;
    let buffer_size = via.macro_buffer_size().map_err(via_error)?;
    let buffer = via.macro_buffer(buffer_size).map_err(via_error)?;
    let macros = (0..macro_count)
        .zip(slots(&buffer, macro_count))
        .map(|(slot, bytes)| MacroSlot {
            slot,
            content: content(&bytes),
            bytes,
        })
        .collect();
    Ok(MacroList {
        macro_count,
        buffer_size,
        macros_enabled: buffer.last() == Some(&0),
        macros,
    })
}

fn content(bytes: &[u8]) -> MacroContent {
    match decode_macro(bytes) {
        Ok(actions) => MacroContent::Actions {
            actions: actions.into_iter().map(Action::from).collect(),
        },
        Err(error) => MacroContent::Unreadable {
            error: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, MacroContent, MacroSlot, read};
    use crate::test_support::{lend, replay};

    #[test]
    fn decodes_each_slot_into_actions() {
        let mut replay = replay("cli-macro-list.toml");
        let list = read(lend(&mut replay.transport), &replay.device).unwrap();
        assert_eq!(
            (list.macro_count, list.buffer_size, list.macros_enabled),
            (4, 40, true)
        );
        assert_eq!(
            list.macros[0],
            MacroSlot {
                slot: 0,
                bytes: vec![104, 101, 108, 108, 111, 1, 1, 40],
                content: MacroContent::Actions {
                    actions: vec![Action::Text("hello".to_owned()), Action::Tap(0x28)]
                },
            }
        );
        assert_eq!(
            list.macros[1].content,
            MacroContent::Actions { actions: vec![] }
        );
        assert_eq!(
            list.macros[2].content,
            MacroContent::Actions {
                actions: vec![
                    Action::Down(0xe1),
                    Action::Text("a".to_owned()),
                    Action::Up(0xe1),
                    Action::Delay(100),
                ]
            }
        );
        assert_eq!(replay.finish(), vec![]);
    }

    #[test]
    fn keeps_the_bytes_of_an_unreadable_slot_and_says_why() {
        let mut replay = replay("cli-macro-list.toml");
        let list = read(lend(&mut replay.transport), &replay.device).unwrap();
        assert_eq!(list.macros[3].bytes, [1, 5, 120]);
        let MacroContent::Unreadable { error } = &list.macros[3].content else {
            panic!("slot 3 decoded: {:?}", list.macros[3].content);
        };
        assert!(error.contains("unknown code 5"), "{error}");
        assert_eq!(replay.finish(), vec![]);
    }
}
