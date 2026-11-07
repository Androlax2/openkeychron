//! Macro text typed as keystrokes for the host's keyboard layout, instead of the firmware's
//! US-ANSI `ascii_to_keycode_lut`. Each character becomes taps on the key that types it,
//! wrapped in Shift or AltGr when it needs one.

use crate::macros::{MacroAction, MacroError};

/// A host keyboard layout that macro text can be typed for.
#[derive(Debug)]
pub struct TextLayout {
    pub name: &'static str,
    keys: &'static [(char, Keystroke)],
    dead_keys: &'static [char],
}

/// What a step of a macro types on a host with a given [`TextLayout`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedStep {
    /// Keystrokes that type exactly this text.
    Text(String),
    /// Any other action, including firmware text.
    Action(MacroAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Keystroke {
    modifier: Option<u8>,
    keycode: u8,
}

/// `KC_LEFT_SHIFT` and `KC_RIGHT_ALT` (`KC_ALGR`) in
/// <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/keycodes.h>.
const SHIFT: u8 = 0xe1;
const ALT_GR: u8 = 0xe6;

const fn plain(keycode: u8) -> Keystroke {
    Keystroke {
        modifier: None,
        keycode,
    }
}

const fn shift(keycode: u8) -> Keystroke {
    Keystroke {
        modifier: Some(SHIFT),
        keycode,
    }
}

const fn alt_gr(keycode: u8) -> Keystroke {
    Keystroke {
        modifier: Some(ALT_GR),
        keycode,
    }
}

/// French AZERTY, as QMK's `keymap_french.h` defines it. The keys and dead keys come from
/// <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/keymap_extras/keymap_french.h>;
/// its `sendstring_french.h` marks the same dead keys. Space is the same key on every layout.
pub const FRENCH_AZERTY: TextLayout = TextLayout {
    name: "French AZERTY",
    keys: &[
        (' ', plain(0x2c)),
        ('²', plain(0x35)),
        ('&', plain(0x1e)),
        ('é', plain(0x1f)),
        ('"', plain(0x20)),
        ('\'', plain(0x21)),
        ('(', plain(0x22)),
        ('-', plain(0x23)),
        ('è', plain(0x24)),
        ('_', plain(0x25)),
        ('ç', plain(0x26)),
        ('à', plain(0x27)),
        (')', plain(0x2d)),
        ('=', plain(0x2e)),
        ('$', plain(0x30)),
        ('ù', plain(0x34)),
        ('*', plain(0x32)),
        ('<', plain(0x64)),
        (',', plain(0x10)),
        (';', plain(0x36)),
        (':', plain(0x37)),
        ('!', plain(0x38)),
        ('a', plain(0x14)),
        ('b', plain(0x05)),
        ('c', plain(0x06)),
        ('d', plain(0x07)),
        ('e', plain(0x08)),
        ('f', plain(0x09)),
        ('g', plain(0x0a)),
        ('h', plain(0x0b)),
        ('i', plain(0x0c)),
        ('j', plain(0x0d)),
        ('k', plain(0x0e)),
        ('l', plain(0x0f)),
        ('m', plain(0x33)),
        ('n', plain(0x11)),
        ('o', plain(0x12)),
        ('p', plain(0x13)),
        ('q', plain(0x04)),
        ('r', plain(0x15)),
        ('s', plain(0x16)),
        ('t', plain(0x17)),
        ('u', plain(0x18)),
        ('v', plain(0x19)),
        ('w', plain(0x1d)),
        ('x', plain(0x1b)),
        ('y', plain(0x1c)),
        ('z', plain(0x1a)),
        ('1', shift(0x1e)),
        ('2', shift(0x1f)),
        ('3', shift(0x20)),
        ('4', shift(0x21)),
        ('5', shift(0x22)),
        ('6', shift(0x23)),
        ('7', shift(0x24)),
        ('8', shift(0x25)),
        ('9', shift(0x26)),
        ('0', shift(0x27)),
        ('°', shift(0x2d)),
        ('+', shift(0x2e)),
        ('£', shift(0x30)),
        ('%', shift(0x34)),
        ('µ', shift(0x32)),
        ('>', shift(0x64)),
        ('?', shift(0x10)),
        ('.', shift(0x36)),
        ('/', shift(0x37)),
        ('§', shift(0x38)),
        ('A', shift(0x14)),
        ('B', shift(0x05)),
        ('C', shift(0x06)),
        ('D', shift(0x07)),
        ('E', shift(0x08)),
        ('F', shift(0x09)),
        ('G', shift(0x0a)),
        ('H', shift(0x0b)),
        ('I', shift(0x0c)),
        ('J', shift(0x0d)),
        ('K', shift(0x0e)),
        ('L', shift(0x0f)),
        ('M', shift(0x33)),
        ('N', shift(0x11)),
        ('O', shift(0x12)),
        ('P', shift(0x13)),
        ('Q', shift(0x04)),
        ('R', shift(0x15)),
        ('S', shift(0x16)),
        ('T', shift(0x17)),
        ('U', shift(0x18)),
        ('V', shift(0x19)),
        ('W', shift(0x1d)),
        ('X', shift(0x1b)),
        ('Y', shift(0x1c)),
        ('Z', shift(0x1a)),
        ('#', alt_gr(0x20)),
        ('{', alt_gr(0x21)),
        ('[', alt_gr(0x22)),
        ('|', alt_gr(0x23)),
        ('\\', alt_gr(0x25)),
        ('@', alt_gr(0x27)),
        (']', alt_gr(0x2d)),
        ('}', alt_gr(0x2e)),
        ('€', alt_gr(0x08)),
        ('¤', alt_gr(0x30)),
    ],
    dead_keys: &['^', '¨', '~', '`'],
};

impl TextLayout {
    /// Keystrokes that type `text` on this layout. Refuses a character the layout cannot
    /// type with one key and its modifiers, rather than send keys that type something else.
    /// A run of characters that need the same modifier holds it once.
    pub fn actions_for_text(&self, text: &str) -> Result<Vec<MacroAction>, MacroError> {
        if text.is_empty() {
            return Err(MacroError::EmptyText);
        }
        let mut actions = Vec::new();
        let mut held = None;
        for character in text.chars() {
            let keystroke = self.keystroke(character)?;
            if keystroke.modifier != held {
                actions.extend(held.map(MacroAction::Up));
                actions.extend(keystroke.modifier.map(MacroAction::Down));
                held = keystroke.modifier;
            }
            actions.push(MacroAction::Tap(keystroke.keycode));
        }
        actions.extend(held.map(MacroAction::Up));
        Ok(actions)
    }

    /// The macro as this layout types it. Tap, down and up actions become text when they are
    /// keystrokes this layout types characters with; everything else stays an action. Once a
    /// key stays held, later taps stay actions too: they type with that key still down.
    #[must_use]
    pub fn typed_steps(&self, actions: &[MacroAction]) -> Vec<TypedStep> {
        let mut steps = Vec::new();
        let mut held_keys = Vec::new();
        let mut index = 0;
        while let Some(action) = actions.get(index) {
            if held_keys.is_empty()
                && let Some((text, length)) = self.typed_run(&actions[index..])
            {
                match steps.last_mut() {
                    Some(TypedStep::Text(previous)) => previous.push_str(&text),
                    _ => steps.push(TypedStep::Text(text)),
                }
                index += length;
                continue;
            }
            match action {
                MacroAction::Down(key) => held_keys.push(*key),
                MacroAction::Up(key) => held_keys.retain(|held| held != key),
                _ => {}
            }
            steps.push(TypedStep::Action(action.clone()));
            index += 1;
        }
        steps
    }

    /// The text at the start of `actions` and how many actions type it: one plain tap, or a
    /// modifier press, taps that each type a character with it, and its release.
    fn typed_run(&self, actions: &[MacroAction]) -> Option<(String, usize)> {
        match actions.first()? {
            MacroAction::Tap(keycode) => Some((self.character(plain(*keycode))?.to_string(), 1)),
            MacroAction::Down(modifier) => {
                let characters: String = actions[1..]
                    .iter()
                    .map_while(|action| match action {
                        MacroAction::Tap(keycode) => self.character(Keystroke {
                            modifier: Some(*modifier),
                            keycode: *keycode,
                        }),
                        _ => None,
                    })
                    .collect();
                let taps = characters.chars().count();
                let released = actions.get(1 + taps) == Some(&MacroAction::Up(*modifier));
                (taps > 0 && released).then_some((characters, taps + 2))
            }
            _ => None,
        }
    }

    fn keystroke(&self, character: char) -> Result<Keystroke, MacroError> {
        if self.dead_keys.contains(&character) {
            return Err(MacroError::DeadKeyText {
                character,
                layout: self.name,
            });
        }
        self.keys
            .iter()
            .find(|(typed, _)| *typed == character)
            .map(|(_, keystroke)| *keystroke)
            .ok_or(MacroError::UntypableText {
                character,
                layout: self.name,
            })
    }

    fn character(&self, keystroke: Keystroke) -> Option<char> {
        self.keys
            .iter()
            .find(|(_, typed)| *typed == keystroke)
            .map(|(character, _)| *character)
    }
}

#[cfg(test)]
mod tests {
    use super::{FRENCH_AZERTY, TypedStep};
    use crate::macros::{MacroAction, MacroError, decode_macro, encode_macro};

    const SHIFT: u8 = 0xe1;
    const ALT_GR: u8 = 0xe6;

    fn french(text: &str) -> Result<Vec<MacroAction>, MacroError> {
        FRENCH_AZERTY.actions_for_text(text)
    }

    fn taps(keycodes: &[u8]) -> Vec<MacroAction> {
        keycodes.iter().copied().map(MacroAction::Tap).collect()
    }

    #[test]
    fn types_letters_on_their_azerty_keys() {
        assert_eq!(french("aqzwm"), Ok(taps(&[0x14, 0x04, 0x1a, 0x1d, 0x33])));
    }

    #[test]
    fn types_french_letters_and_punctuation_without_a_modifier() {
        assert_eq!(french("é ç,!"), Ok(taps(&[0x1f, 0x2c, 0x26, 0x10, 0x38])));
    }

    #[test]
    fn holds_shift_once_across_a_run_of_shifted_characters() {
        assert_eq!(
            french("A12b"),
            Ok(vec![
                MacroAction::Down(SHIFT),
                MacroAction::Tap(0x14),
                MacroAction::Tap(0x1e),
                MacroAction::Tap(0x1f),
                MacroAction::Up(SHIFT),
                MacroAction::Tap(0x05),
            ])
        );
    }

    #[test]
    fn types_alt_gr_characters_with_right_alt() {
        assert_eq!(
            french("@#€"),
            Ok(vec![
                MacroAction::Down(ALT_GR),
                MacroAction::Tap(0x27),
                MacroAction::Tap(0x20),
                MacroAction::Tap(0x08),
                MacroAction::Up(ALT_GR),
            ])
        );
    }

    #[test]
    fn releases_one_modifier_before_pressing_the_next() {
        assert_eq!(
            french("1@"),
            Ok(vec![
                MacroAction::Down(SHIFT),
                MacroAction::Tap(0x1e),
                MacroAction::Up(SHIFT),
                MacroAction::Down(ALT_GR),
                MacroAction::Tap(0x27),
                MacroAction::Up(ALT_GR),
            ])
        );
    }

    #[test]
    fn refuses_characters_that_need_a_dead_key() {
        for character in ['^', '¨', '~', '`'] {
            assert_eq!(
                french(&format!("a{character}")),
                Err(MacroError::DeadKeyText {
                    character,
                    layout: "French AZERTY"
                })
            );
        }
    }

    #[test]
    fn refuses_characters_the_layout_has_no_key_for() {
        for character in ['ñ', 'ß', '\n', '\t'] {
            assert_eq!(
                french(&character.to_string()),
                Err(MacroError::UntypableText {
                    character,
                    layout: "French AZERTY"
                })
            );
        }
    }

    #[test]
    fn refuses_empty_text() {
        assert_eq!(french(""), Err(MacroError::EmptyText));
    }

    #[test]
    fn reads_back_every_character_it_can_type() {
        for &(character, _) in FRENCH_AZERTY.keys {
            let actions = french(&character.to_string()).unwrap();
            assert_eq!(
                FRENCH_AZERTY.typed_steps(&actions),
                vec![TypedStep::Text(character.to_string())],
                "{character:?}"
            );
        }
    }

    #[test]
    fn gives_each_character_one_keystroke_and_each_keystroke_one_character() {
        for (index, (character, keystroke)) in FRENCH_AZERTY.keys.iter().enumerate() {
            for (other_character, other_keystroke) in &FRENCH_AZERTY.keys[index + 1..] {
                assert_ne!(character, other_character);
                assert_ne!(
                    keystroke, other_keystroke,
                    "{character:?} {other_character:?}"
                );
            }
        }
    }

    #[test]
    fn reads_back_text_from_the_bytes_written_to_the_keyboard() {
        let bytes = encode_macro(&french("Aé1@ ok").unwrap()).unwrap();
        let actions = decode_macro(&bytes).unwrap();
        assert_eq!(
            FRENCH_AZERTY.typed_steps(&actions),
            vec![TypedStep::Text("Aé1@ ok".to_owned())]
        );
    }

    #[test]
    fn reads_back_mixed_text_as_one_string() {
        let actions = french("Bonjour, à 10h @Théo !").unwrap();
        assert_eq!(
            FRENCH_AZERTY.typed_steps(&actions),
            vec![TypedStep::Text("Bonjour, à 10h @Théo !".to_owned())]
        );
    }

    #[test]
    fn keeps_actions_that_type_no_character_on_the_layout() {
        let actions = vec![
            MacroAction::Tap(0x0b),
            MacroAction::Tap(0x0c),
            MacroAction::Tap(0x28),
            MacroAction::Delay(5),
            MacroAction::Text("a".to_owned()),
            MacroAction::Tap(0x2f),
        ];
        assert_eq!(
            FRENCH_AZERTY.typed_steps(&actions),
            vec![
                TypedStep::Text("hi".to_owned()),
                TypedStep::Action(MacroAction::Tap(0x28)),
                TypedStep::Action(MacroAction::Delay(5)),
                TypedStep::Action(MacroAction::Text("a".to_owned())),
                TypedStep::Action(MacroAction::Tap(0x2f)),
            ]
        );
    }

    #[test]
    fn keeps_a_modifier_hold_that_does_not_only_type_characters() {
        let cases = [
            vec![
                MacroAction::Down(SHIFT),
                MacroAction::Tap(0x28),
                MacroAction::Up(SHIFT),
            ],
            vec![MacroAction::Down(SHIFT), MacroAction::Tap(0x14)],
            vec![MacroAction::Down(SHIFT), MacroAction::Up(SHIFT)],
            vec![
                MacroAction::Down(ALT_GR),
                MacroAction::Tap(0x1f),
                MacroAction::Up(ALT_GR),
            ],
            vec![
                MacroAction::Down(SHIFT),
                MacroAction::Tap(0x14),
                MacroAction::Up(ALT_GR),
                MacroAction::Tap(0x14),
            ],
            vec![
                MacroAction::Down(0x04),
                MacroAction::Tap(0x05),
                MacroAction::Up(0x04),
            ],
        ];
        for actions in cases {
            assert_eq!(
                FRENCH_AZERTY.typed_steps(&actions),
                actions
                    .iter()
                    .cloned()
                    .map(TypedStep::Action)
                    .collect::<Vec<_>>(),
                "{actions:?}"
            );
        }
    }
}
