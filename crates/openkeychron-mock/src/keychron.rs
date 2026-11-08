//! The Keychron requests the mock keyboard answers: `0xA0`, `0xA1`, `0xA2`, the misc group's
//! debounce, Snap Click and wireless timers, and the RGB group (`crate::lighting`). IDs and
//! layouts from `docs/protocol/keychron.md`, which cites `keyboards/keychron/common/keychron_raw_hid.h` and its handlers in
//! <https://github.com/Keychron/qmk_firmware/tree/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/common>.

use std::ops::Range;

use openkeychron_via::REPORT_LEN;

use crate::keyboard::{State, not_answered};
use crate::lighting;

const GET_PROTOCOL_VERSION: u8 = 0xa0;
const GET_FIRMWARE_VERSION: u8 = 0xa1;
const GET_SUPPORT_FEATURE: u8 = 0xa2;
const MISC_GROUP: u8 = 0xa7;
const RGB_GROUP: u8 = 0xa8;

const MISC_GET_PROTOCOL_VER: u8 = 0x01;
const DEBOUNCE_GET: u8 = 0x05;
const DEBOUNCE_SET: u8 = 0x06;
const SNAP_CLICK_GET_INFO: u8 = 0x07;
const SNAP_CLICK_GET: u8 = 0x08;
const SNAP_CLICK_SET: u8 = 0x09;
const SNAP_CLICK_SAVE: u8 = 0x0a;
const WIRELESS_LPM_GET: u8 = 0x0b;
const WIRELESS_LPM_SET: u8 = 0x0c;

/// `0xA0`'s protocol version and `QMK_COMMAND_SET`.
const PROTOCOL_VERSION: u8 = 0x02;
const COMMAND_SET: u8 = 2;
/// `v<version> <build date>`, the date naming it a mock so the UI never passes it off as a
/// real firmware.
const FIRMWARE_VERSION: &[u8] = b"v0.0.0 mock";
/// Dynamic debounce (bit 5), Snap Click (bit 6) and Keychron RGB (bit 7): only the features the
/// mock answers, so a host never sends it a request for the others.
const FEATURES: u16 = 0b1110_0000;
const MISC_PROTOCOL_VERSION: u16 = 0x0002;
/// Debounce (bit 2), Snap Click (bit 3) and wireless timers (bit 4).
const MISC_FEATURES: u8 = 0b0001_1100;

const SUCCESS: u8 = 0;
const FAILURE: u8 = 1;
/// The firmware's algorithms are 0 to 6 (`common/debounce/keychron_debounce.h`).
const DEBOUNCE_ALGORITHMS: u8 = 7;
/// Algorithm 5, whose time the firmware caps at 127 ms (`docs/protocol/keychron.md`).
const ASYMMETRIC: u8 = 5;
const ASYMMETRIC_MAX_MS: u8 = 127;
/// Snap Click modes are 0 to 5 (`common/snap_click/snap_click.c`).
const SNAP_CLICK_MODES: u8 = 6;
const SNAP_CLICK_PAIRS_PER_REQUEST: u8 = 9;
/// `wireless_lpm_set` refuses shorter times (`common/wireless/wireless.c`).
const MIN_BACKLIGHT_OFF_SECONDS: u16 = 5;
const MIN_SLEEP_SECONDS: u16 = 60;

/// Answers one Keychron request in `reply`, which starts as the request.
pub(crate) fn answer(state: &mut State, reply: &mut [u8; REPORT_LEN]) -> Result<(), String> {
    match reply[0] {
        GET_PROTOCOL_VERSION => {
            reply[1..4].copy_from_slice(&[PROTOCOL_VERSION, 0, COMMAND_SET]);
            Ok(())
        }
        GET_FIRMWARE_VERSION => {
            reply[1..].fill(0);
            reply[1..=FIRMWARE_VERSION.len()].copy_from_slice(FIRMWARE_VERSION);
            Ok(())
        }
        GET_SUPPORT_FEATURE => {
            reply[1] = 0;
            reply[2..4].copy_from_slice(&FEATURES.to_le_bytes());
            Ok(())
        }
        MISC_GROUP => misc(state, reply),
        RGB_GROUP => lighting::keychron_rgb(&mut state.lighting, reply),
        _ => Err(not_answered(reply)),
    }
}

/// `A7 <sub>`: the reply's byte 2 is the status, its data starts at byte 3.
fn misc(state: &mut State, reply: &mut [u8; REPORT_LEN]) -> Result<(), String> {
    let sub = reply[1];
    let refuse = |reason: String| {
        format!("the mock keyboard refuses Keychron command 0xa7 0x{sub:02x}: {reason}")
    };
    let status = match sub {
        MISC_GET_PROTOCOL_VER => {
            reply[3..5].copy_from_slice(&MISC_PROTOCOL_VERSION.to_le_bytes());
            reply[5] = MISC_FEATURES;
            SUCCESS
        }
        DEBOUNCE_GET => {
            reply[3..6].copy_from_slice(&[0, state.debounce[0], state.debounce[1]]);
            SUCCESS
        }
        DEBOUNCE_SET if reply[2] >= DEBOUNCE_ALGORITHMS => FAILURE,
        DEBOUNCE_SET if reply[3] == 0 => {
            return Err(refuse(
                "the firmware turns a time of 0 ms into no debounce, which the mock does not \
                 model"
                    .to_owned(),
            ));
        }
        DEBOUNCE_SET if reply[2] == ASYMMETRIC && reply[3] > ASYMMETRIC_MAX_MS => {
            return Err(refuse(format!(
                "the firmware caps the asymmetric algorithm at {ASYMMETRIC_MAX_MS} ms, which the \
                 mock does not model"
            )));
        }
        DEBOUNCE_SET => {
            state.debounce = [reply[2], reply[3]];
            SUCCESS
        }
        SNAP_CLICK_GET_INFO => {
            reply[3] = u8::try_from(state.snap_click.len())
                .map_err(|_| refuse("it holds too many pairs to report".to_owned()))?;
            SUCCESS
        }
        SNAP_CLICK_GET => {
            let pairs = snap_click_pairs(state, reply).map_err(refuse)?;
            let (entries, _) = reply[3..].as_chunks_mut::<3>();
            for (entry, pair) in entries.iter_mut().zip(&state.snap_click[pairs]) {
                *entry = *pair;
            }
            SUCCESS
        }
        SNAP_CLICK_SET => {
            let pairs = snap_click_pairs(state, reply).map_err(refuse)?;
            let entries = &reply[4..].as_chunks::<3>().0[..pairs.len()];
            if let Some([mode, ..]) = entries.iter().find(|[mode, ..]| *mode >= SNAP_CLICK_MODES) {
                return Err(refuse(format!("Snap Click mode {mode} is unknown")));
            }
            state.snap_click[pairs].copy_from_slice(entries);
            SUCCESS
        }
        // The fake keeps one copy of the pairs: saving them to EEPROM changes nothing it reports.
        SNAP_CLICK_SAVE => SUCCESS,
        WIRELESS_LPM_GET => {
            reply[3..5].copy_from_slice(&state.wireless[0].to_le_bytes());
            reply[5..7].copy_from_slice(&state.wireless[1].to_le_bytes());
            SUCCESS
        }
        WIRELESS_LPM_SET => {
            let backlight_off = u16::from_le_bytes([reply[2], reply[3]]);
            let sleep = u16::from_le_bytes([reply[4], reply[5]]);
            if backlight_off < MIN_BACKLIGHT_OFF_SECONDS || sleep < MIN_SLEEP_SECONDS {
                FAILURE
            } else {
                state.wireless = [backlight_off, sleep];
                SUCCESS
            }
        }
        _ => return Err(not_answered(reply)),
    };
    reply[2] = status;
    Ok(())
}

/// The pairs a Snap Click request names: start `[2]`, count `[3]`.
fn snap_click_pairs(state: &State, request: &[u8; REPORT_LEN]) -> Result<Range<usize>, String> {
    let [start, count] = [request[2], request[3]];
    if count > SNAP_CLICK_PAIRS_PER_REQUEST {
        return Err(format!(
            "{count} pairs is more than the {SNAP_CLICK_PAIRS_PER_REQUEST} one request carries"
        ));
    }
    let end = usize::from(start) + usize::from(count);
    if end > state.snap_click.len() {
        return Err(format!(
            "pairs {start}..{end} are outside its {} pairs",
            state.snap_click.len()
        ));
    }
    Ok(usize::from(start)..end)
}
