//! The lighting the mock keyboard holds and answers: VIA's RGB matrix values (`07`, `08`, `09` on
//! channel 3) and Keychron's RGB group (`0xA8`) for per-key and mixed RGB. Layouts from
//! `docs/protocol/keychron.md`; behaviour from `via_qmk_rgb_matrix_command` in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.c>,
//! the `rgb_matrix_*_eeprom_helper` setters in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/rgb_matrix/rgb_matrix.c>
//! and `kc_rgb_matrix_rx` in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/common/rgb/keychron_rgb.c>.
//!
//! The fake keeps one copy of everything, so the saves (`09 03`, `A8 02`) change nothing it
//! reports, and VIA's "one value per save" rule cannot show.

use std::ops::Range;

use openkeychron_via::REPORT_LEN;

use crate::keyboard::not_answered;

const CUSTOM_SET_VALUE: u8 = 0x07;
const CUSTOM_GET_VALUE: u8 = 0x08;
const CUSTOM_SAVE: u8 = 0x09;
/// `id_qmk_rgb_matrix_channel`.
const RGB_MATRIX_CHANNEL: u8 = 3;
const BRIGHTNESS: u8 = 1;
const EFFECT: u8 = 2;
const EFFECT_SPEED: u8 = 3;
const COLOR: u8 = 4;

const GET_PROTOCOL_VERSION: u8 = 0x01;
const SAVE: u8 = 0x02;
const GET_LED_COUNT: u8 = 0x05;
const GET_LED_INDEX: u8 = 0x06;
const GET_COLORS: u8 = 0x09;
const SET_COLORS: u8 = 0x0a;
const MIXED_GET_INFO: u8 = 0x0b;
const GET_REGIONS: u8 = 0x0c;
const SET_REGIONS: u8 = 0x0d;
const GET_EFFECT_LIST: u8 = 0x0e;
const SET_EFFECT_LIST: u8 = 0x0f;

/// `PER_KEY_RGB_VER`.
const RGB_PROTOCOL_VERSION: u16 = 0x0001;
const SUCCESS: u8 = 0;
const FAILURE: u8 = 1;

/// Synthetic: effect numbers depend on the firmware build (`docs/protocol/keychron.md`). Solid
/// colour, then `v5_max/info.json`'s 21 animations, then Keychron's `PER_KEY_RGB` and
/// `MIXED_RGB` (`common/rgb/rgb_matrix_kb.inc`) would make mixed RGB 24, but no V5 Max has
/// reported it to openkeychron.
const MIXED_RGB_EFFECT: u8 = 24;
/// `RGB_MATRIX_EFFECT_MAX`: mixed RGB is the last effect.
const EFFECT_COUNT: u8 = MIXED_RGB_EFFECT + 1;
/// `RGB_MATRIX_SOLID_COLOR`, the first effect of every build.
const SOLID_COLOR: u8 = 1;

/// `RGB_MATRIX_LED_COUNT` in `v5_max/iso_encoder/config.h`.
const LED_COUNT: usize = 98;
const NO_LED: u8 = 0xff;
/// `g_led_config`'s key matrix to LED index in `v5_max/iso_encoder/iso_encoder.c`. Only the V5
/// Max ISO has a built-in layout, so `MockKeyboard::new` makes no other model; a second built-in
/// model needs its own map.
const LED_UNDER_KEY: [[u8; 19]; 6] = {
    const N: u8 = NO_LED;
    [
        [
            0, N, 1, 2, 3, 4, N, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, N,
        ],
        [
            16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, N, 30, 31, 32, 33,
        ],
        [
            34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, N, 48, 49, 50, N,
        ],
        [
            51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, N, N, 64, 65, 66, 67,
        ],
        [
            68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, N, 81, 82, 83, 84, 85,
        ],
        [
            86, 87, 88, N, N, N, 89, N, N, N, 90, 91, 92, 93, 94, 95, 96, 97, N,
        ],
    ]
};
/// `EFFECT_LAYERS` and `EFFECTS_PER_LAYER` (`common/rgb/rgb_matrix_kb_config.h`).
const REGION_COUNT: u8 = 2;
const EFFECTS_PER_REGION: u8 = 5;
/// `EFFECT_DATA_LEN`: effect, H, S, speed, then the duration as a little-endian u32.
const EFFECT_LEN: usize = 8;

/// The handlers' limits on entries per request.
const COLORS_PER_REQUEST: u8 = 9;
const REGIONS_PER_GET: u8 = 29;
const REGIONS_PER_SET: u8 = 28;
const EFFECTS_PER_REQUEST: u8 = 3;

/// What the fake keyboard's lighting holds. The seed is synthetic, like the keymap's: zero
/// everywhere, not the V5 Max's firmware defaults, except an effect, without which the firmware
/// ignores every other VIA value.
#[derive(Debug)]
pub(crate) struct Lighting {
    /// The effect VIA reports: 0 while lighting is off.
    effect: u8,
    /// The stored brightness, after the firmware's scaling.
    brightness: u8,
    speed: u8,
    hue: u8,
    saturation: u8,
    /// H, S, V per LED.
    colors: [[u8; 3]; LED_COUNT],
    /// The mixed RGB region of each LED.
    regions: [u8; LED_COUNT],
    effect_lists: [[[u8; EFFECT_LEN]; EFFECTS_PER_REGION as usize]; REGION_COUNT as usize],
}

impl Lighting {
    pub(crate) fn seeded() -> Self {
        Self {
            effect: SOLID_COLOR,
            brightness: 0,
            speed: 0,
            hue: 0,
            saturation: 0,
            colors: [[0; 3]; LED_COUNT],
            regions: [0; LED_COUNT],
            effect_lists: [[[0; EFFECT_LEN]; EFFECTS_PER_REGION as usize]; REGION_COUNT as usize],
        }
    }
}

/// Answers `07`, `08` or `09` on VIA's RGB matrix channel in `reply`, which starts as the
/// request. Every other channel is not answered.
pub(crate) fn via_rgb_matrix(
    lighting: &mut Lighting,
    reply: &mut [u8; REPORT_LEN],
) -> Result<(), String> {
    let [command, channel, value_id] = [reply[0], reply[1], reply[2]];
    if channel != RGB_MATRIX_CHANNEL {
        return Err(not_answered(reply));
    }
    if command == CUSTOM_SAVE {
        return Ok(());
    }
    if !(BRIGHTNESS..=COLOR).contains(&value_id) {
        return Err(not_answered(reply));
    }
    let refuse =
        |reason: String| format!("the mock keyboard refuses VIA command 0x{command:02x}: {reason}");
    match command {
        CUSTOM_GET_VALUE => match value_id {
            BRIGHTNESS => reply[3] = lighting.brightness,
            EFFECT => reply[3] = lighting.effect,
            EFFECT_SPEED => reply[3] = lighting.speed,
            _ => reply[3..5].copy_from_slice(&[lighting.hue, lighting.saturation]),
        },
        CUSTOM_SET_VALUE if value_id == EFFECT => {
            let effect = reply[3];
            if effect >= EFFECT_COUNT {
                return Err(refuse(format!(
                    "the firmware clamps effect {effect} to its last effect, {}, which the mock \
                     does not model",
                    EFFECT_COUNT - 1
                )));
            }
            lighting.effect = effect;
        }
        // `rgb_matrix_sethsv_noeeprom` and `rgb_matrix_set_speed_noeeprom` return early while
        // lighting is disabled.
        CUSTOM_SET_VALUE if lighting.effect == 0 => {
            return Err(refuse(
                "the firmware ignores brightness, speed and colour while lighting is off, which \
                 the mock does not model"
                    .to_owned(),
            ));
        }
        CUSTOM_SET_VALUE => match value_id {
            BRIGHTNESS => lighting.brightness = stored_brightness(reply[3]),
            EFFECT_SPEED => lighting.speed = reply[3],
            _ => [lighting.hue, lighting.saturation] = [reply[3], reply[4]],
        },
        _ => return Err(not_answered(reply)),
    }
    Ok(())
}

/// `scale8(value, RGB_MATRIX_MAXIMUM_BRIGHTNESS)` with the V5 Max's maximum of 255, without the
/// "fixed" rounding (`lib/lib8tion/scale8.h`): `(value × 255) >> 8`.
fn stored_brightness(value: u8) -> u8 {
    let [_, scaled] = ((u16::from(value) * 255) >> 8).to_be_bytes();
    scaled
}

/// `A8 <sub>`: the reply's byte 2 is the status, its data starts at byte 3.
pub(crate) fn keychron_rgb(
    lighting: &mut Lighting,
    reply: &mut [u8; REPORT_LEN],
) -> Result<(), String> {
    let sub = reply[1];
    let refuse = |reason: String| {
        format!("the mock keyboard refuses Keychron command 0xa8 0x{sub:02x}: {reason}")
    };
    let status = match sub {
        GET_PROTOCOL_VERSION => {
            reply[3..5].copy_from_slice(&RGB_PROTOCOL_VERSION.to_le_bytes());
            SUCCESS
        }
        SAVE => SUCCESS,
        GET_LED_COUNT => {
            reply[3] = u8::try_from(LED_COUNT)
                .map_err(|_| refuse("it holds too many LEDs to report".to_owned()))?;
            SUCCESS
        }
        GET_LED_INDEX => led_index(reply).map_err(refuse)?,
        GET_COLORS | SET_COLORS if reply[3] > COLORS_PER_REQUEST => FAILURE,
        GET_COLORS => {
            let leds = led_range(reply[2], reply[3]).map_err(refuse)?;
            let (entries, _) = reply[3..].as_chunks_mut::<3>();
            for (entry, color) in entries.iter_mut().zip(&lighting.colors[leds]) {
                *entry = *color;
            }
            SUCCESS
        }
        SET_COLORS => {
            let leds = led_range(reply[2], reply[3]).map_err(refuse)?;
            let entries = &reply[4..].as_chunks::<3>().0[..leds.len()];
            lighting.colors[leds].copy_from_slice(entries);
            SUCCESS
        }
        MIXED_GET_INFO => {
            reply[3..5].copy_from_slice(&[REGION_COUNT, EFFECTS_PER_REGION]);
            SUCCESS
        }
        GET_REGIONS => match led_range_within(reply[2], reply[3], REGIONS_PER_GET) {
            Some(leds) => {
                reply[3..3 + leds.len()].copy_from_slice(&lighting.regions[leds]);
                SUCCESS
            }
            None => FAILURE,
        },
        SET_REGIONS => match led_range_within(reply[2], reply[3], REGIONS_PER_SET) {
            Some(leds) => {
                let regions = &reply[4..4 + leds.len()];
                if regions.iter().any(|region| *region >= REGION_COUNT) {
                    FAILURE
                } else {
                    lighting.regions[leds].copy_from_slice(regions);
                    SUCCESS
                }
            }
            None => FAILURE,
        },
        GET_EFFECT_LIST => match effect_slots(reply) {
            Some((region, slots)) => {
                let region = region_index(region).map_err(refuse)?;
                let (entries, _) = reply[3..].as_chunks_mut::<EFFECT_LEN>();
                for (entry, slot) in entries
                    .iter_mut()
                    .zip(&lighting.effect_lists[region][slots])
                {
                    *entry = *slot;
                }
                SUCCESS
            }
            None => FAILURE,
        },
        SET_EFFECT_LIST => match effect_slots(reply) {
            Some((region, slots)) => {
                let entries = &reply[5..].as_chunks::<EFFECT_LEN>().0[..slots.len()];
                // The firmware checks the effects before it writes the region's slots.
                if entries
                    .iter()
                    .any(|[effect, ..]| *effect >= MIXED_RGB_EFFECT)
                {
                    FAILURE
                } else {
                    let region = region_index(region).map_err(refuse)?;
                    lighting.effect_lists[region][slots].copy_from_slice(entries);
                    SUCCESS
                }
            }
            None => FAILURE,
        },
        _ => return Err(not_answered(reply)),
    };
    reply[2] = status;
    Ok(())
}

/// `A8 06`: row `[2]`, column mask `[3..6]`. Fills a byte from `[3]` for every column of the
/// matrix: the LED under it, or `0xFF` when it has none or is not in the mask.
fn led_index(reply: &mut [u8; REPORT_LEN]) -> Result<u8, String> {
    let rows = LED_UNDER_KEY.len();
    let row = usize::from(reply[2]);
    // The firmware's check is `row > MATRIX_ROWS`.
    if row > rows {
        return Ok(FAILURE);
    }
    let leds = LED_UNDER_KEY.get(row).ok_or_else(|| {
        format!("row {row} is outside its {rows} rows, which the firmware reads past its matrix")
    })?;
    let mask = u32::from_le_bytes([reply[3], reply[4], reply[5], 0]);
    for (column, led) in leds.iter().enumerate() {
        reply[3 + column] = if mask & (1 << column) == 0 {
            NO_LED
        } else {
            *led
        };
    }
    Ok(SUCCESS)
}

/// The LEDs a per-key colour request names: start `[2]`, count `[3]`, already at most nine. The
/// firmware never checks the end against its LED count.
fn led_range(start: u8, count: u8) -> Result<Range<usize>, String> {
    let leds = usize::from(start)..usize::from(start) + usize::from(count);
    if leds.end > LED_COUNT {
        return Err(format!(
            "LEDs {}..{} are outside its {LED_COUNT} LEDs, which the firmware reads and writes \
             past its array",
            leds.start, leds.end
        ));
    }
    Ok(leds)
}

/// The LEDs a region request names, or `None` where the firmware fails it: more than `limit`,
/// or past the last LED.
fn led_range_within(start: u8, count: u8, limit: u8) -> Option<Range<usize>> {
    let leds = usize::from(start)..usize::from(start) + usize::from(count);
    (count <= limit && leds.end <= LED_COUNT).then_some(leds)
}

/// The region and slots an effect list request names: region `[2]`, start `[3]`, count `[4]`.
/// `None` where the firmware fails it. Its region check is off by one (`region > EFFECT_LAYERS`),
/// so the region may be one past the last: see [`region_index`].
fn effect_slots(request: &[u8; REPORT_LEN]) -> Option<(u8, Range<usize>)> {
    let [region, start, count] = [request[2], request[3], request[4]];
    let slots = usize::from(start)..usize::from(start) + usize::from(count);
    let fails = count > EFFECTS_PER_REQUEST
        || region > REGION_COUNT
        || slots.end > usize::from(EFFECTS_PER_REGION);
    (!fails).then_some((region, slots))
}

/// The index of `region` in the effect lists, refusing the one past the last that the firmware
/// lets through and indexes past its array with.
fn region_index(region: u8) -> Result<usize, String> {
    if region >= REGION_COUNT {
        return Err(format!(
            "region {region} is outside its {REGION_COUNT} regions, which the firmware indexes \
             past its array"
        ));
    }
    Ok(usize::from(region))
}
