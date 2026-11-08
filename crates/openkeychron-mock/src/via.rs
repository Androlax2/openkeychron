//! The VIA requests the mock keyboard answers. IDs from `enum via_command_id` and
//! `enum via_keyboard_value_id` in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.h>;
//! request and reply layouts from `raw_hid_receive` in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.c>.

use std::ops::Range;

use openkeychron_via::REPORT_LEN;

use crate::keyboard::{ENCODER_COUNT, State, not_answered};
use crate::lighting;

const GET_PROTOCOL_VERSION: u8 = 0x01;
const GET_KEYBOARD_VALUE: u8 = 0x02;
const GET_KEYCODE: u8 = 0x04;
const SET_KEYCODE: u8 = 0x05;
const RESET_KEYMAP: u8 = 0x06;
const CUSTOM_SET_VALUE: u8 = 0x07;
const CUSTOM_GET_VALUE: u8 = 0x08;
const CUSTOM_SAVE: u8 = 0x09;
const GET_MACRO_COUNT: u8 = 0x0c;
const GET_MACRO_BUFFER_SIZE: u8 = 0x0d;
const GET_MACRO_BUFFER: u8 = 0x0e;
const SET_MACRO_BUFFER: u8 = 0x0f;
const GET_LAYER_COUNT: u8 = 0x11;
const GET_KEYMAP_BUFFER: u8 = 0x12;
const SET_KEYMAP_BUFFER: u8 = 0x13;
const GET_ENCODER: u8 = 0x14;
const SET_ENCODER: u8 = 0x15;

const UPTIME: u8 = 0x01;
const LAYOUT_OPTIONS: u8 = 0x02;
const FIRMWARE_VERSION: u8 = 0x04;

/// `VIA_PROTOCOL_VERSION` in `via.h`.
const PROTOCOL_VERSION: u16 = 0x000c;
/// Synthetic, like the uptime in the hand-written fixtures.
const UPTIME_MS: u32 = 12_345;

/// The most bytes one buffer request carries (`size <= 28` in `via.c`).
const CHUNK_BYTES: u8 = 28;

/// Answers one VIA request in `reply`, which starts as the request.
pub(crate) fn answer(state: &mut State, reply: &mut [u8; REPORT_LEN]) -> Result<(), String> {
    let command = reply[0];
    let refuse =
        |reason: String| format!("the mock keyboard refuses VIA command 0x{command:02x}: {reason}");
    match command {
        GET_PROTOCOL_VERSION => reply[1..3].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes()),
        GET_KEYBOARD_VALUE => {
            let value = match reply[1] {
                UPTIME => UPTIME_MS,
                // `guard` refuses any other layout option, and the seed has none.
                LAYOUT_OPTIONS | FIRMWARE_VERSION => 0,
                _ => return Err(not_answered(reply)),
            };
            reply[2..6].copy_from_slice(&value.to_be_bytes());
        }
        GET_KEYCODE => {
            let key = key_bytes(state, reply).map_err(refuse)?;
            reply[4..6].copy_from_slice(&state.keymap[key]);
        }
        SET_KEYCODE => {
            let key = key_bytes(state, reply).map_err(refuse)?;
            state.keymap[key].copy_from_slice(&reply[4..6]);
        }
        RESET_KEYMAP => state.reset_keymap(),
        CUSTOM_SET_VALUE | CUSTOM_GET_VALUE | CUSTOM_SAVE => {
            lighting::via_rgb_matrix(&mut state.lighting, reply)?;
        }
        GET_MACRO_COUNT => reply[1] = state.macro_count,
        GET_MACRO_BUFFER_SIZE => {
            let size = u16::try_from(state.macros.len())
                .map_err(|_| refuse("its macro buffer is too large to report".to_owned()))?;
            reply[1..3].copy_from_slice(&size.to_be_bytes());
        }
        GET_MACRO_BUFFER => {
            let bytes = chunk(reply, state.macros.len(), "macro buffer").map_err(refuse)?;
            reply[4..4 + bytes.len()].copy_from_slice(&state.macros[bytes]);
        }
        SET_MACRO_BUFFER => {
            let bytes = chunk(reply, state.macros.len(), "macro buffer").map_err(refuse)?;
            state.macros[bytes.clone()].copy_from_slice(&reply[4..4 + bytes.len()]);
        }
        GET_LAYER_COUNT => reply[1] = state.layer_count,
        GET_KEYMAP_BUFFER => {
            let bytes = chunk(reply, state.keymap.len(), "keymap").map_err(refuse)?;
            reply[4..4 + bytes.len()].copy_from_slice(&state.keymap[bytes]);
        }
        SET_KEYMAP_BUFFER => {
            let bytes = chunk(reply, state.keymap.len(), "keymap").map_err(refuse)?;
            state.keymap[bytes.clone()].copy_from_slice(&reply[4..4 + bytes.len()]);
        }
        GET_ENCODER => {
            let key = encoder_bytes(state, reply).map_err(refuse)?;
            reply[4..6].copy_from_slice(&state.encoders[key]);
        }
        SET_ENCODER => {
            let key = encoder_bytes(state, reply).map_err(refuse)?;
            state.encoders[key].copy_from_slice(&reply[4..6]);
        }
        _ => return Err(not_answered(reply)),
    }
    Ok(())
}

/// The keymap bytes of the key at `[1]` layer, `[2]` row, `[3]` column.
fn key_bytes(state: &State, request: &[u8; REPORT_LEN]) -> Result<Range<usize>, String> {
    let [layer, row, column] = [request[1], request[2], request[3]];
    if layer >= state.layer_count || row >= state.rows || column >= state.columns {
        return Err(format!(
            "layer {layer} row {row} column {column} is outside its {} layers of {} × {} keys",
            state.layer_count, state.rows, state.columns
        ));
    }
    let key = (usize::from(layer) * usize::from(state.rows) + usize::from(row))
        * usize::from(state.columns)
        + usize::from(column);
    Ok(key * 2..key * 2 + 2)
}

/// The encoder bytes of `[1]` layer, `[2]` encoder, `[3]` direction (1 clockwise).
fn encoder_bytes(state: &State, request: &[u8; REPORT_LEN]) -> Result<Range<usize>, String> {
    let [layer, encoder, clockwise] = [request[1], request[2], request[3]];
    if layer >= state.layer_count || encoder >= ENCODER_COUNT || clockwise > 1 {
        return Err(format!(
            "layer {layer} encoder {encoder} direction {clockwise} is outside its {} layers of \
             {ENCODER_COUNT} encoder turning 0 (counter-clockwise) or 1 (clockwise)",
            state.layer_count
        ));
    }
    let key = (usize::from(layer) * usize::from(ENCODER_COUNT) + usize::from(encoder)) * 2
        + usize::from(clockwise);
    Ok(key * 2..key * 2 + 2)
}

/// The bytes a buffer request names: offset `[1..3]` (big-endian) and size `[3]`.
fn chunk(request: &[u8; REPORT_LEN], length: usize, buffer: &str) -> Result<Range<usize>, String> {
    let offset = usize::from(u16::from_be_bytes([request[1], request[2]]));
    let size = request[3];
    if size > CHUNK_BYTES {
        return Err(format!(
            "{size} bytes is more than the {CHUNK_BYTES} one request carries"
        ));
    }
    let end = offset + usize::from(size);
    if end > length {
        return Err(format!(
            "bytes {offset}..{end} are outside its {length}-byte {buffer}"
        ));
    }
    Ok(offset..end)
}
