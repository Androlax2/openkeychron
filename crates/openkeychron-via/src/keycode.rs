//! Compact, lossless views of QMK's 16-bit keycodes. Ranges and extraction masks come from
//! `quantum/keycodes.h` and `quantum/quantum_keycodes.h` at
//! <https://github.com/Keychron/qmk_firmware/tree/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum>.

/// A keycode exactly as stored by the keyboard, including unknown/custom values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keycode(u16);

/// A decoded view, not an encoding API: the original [`Keycode`] remains authoritative.
/// Modifier fields use QMK's five-bit encoding: bit 4 selects right-side modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeycodeKind {
    Basic(u8),
    Modified { modifiers: u8, key: u8 },
    ModTap { modifiers: u8, key: u8 },
    LayerTap { layer: u8, key: u8 },
    LayerMod { layer: u8, modifiers: u8 },
    Layer { action: LayerAction, layer: u8 },
    OneShotModifiers(u8),
    Raw(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerAction {
    To,
    Momentary,
    Default,
    Toggle,
    OneShot,
    TapToggle,
    PersistentDefault,
}

impl From<u16> for Keycode {
    fn from(raw: u16) -> Self {
        Self(raw)
    }
}

impl From<Keycode> for u16 {
    fn from(keycode: Keycode) -> Self {
        keycode.0
    }
}

impl Keycode {
    #[must_use]
    pub fn kind(self) -> KeycodeKind {
        let [key, high] = self.0.to_le_bytes();
        let modifiers = high & 0x1f;
        let layer = key & 0x1f;
        match self.0 {
            0x0000..=0x00ff => KeycodeKind::Basic(key),
            0x0100..=0x1fff => KeycodeKind::Modified { modifiers, key },
            0x2000..=0x3fff => KeycodeKind::ModTap { modifiers, key },
            0x4000..=0x4fff => KeycodeKind::LayerTap {
                layer: high & 0x0f,
                key,
            },
            0x5000..=0x51ff => KeycodeKind::LayerMod {
                layer: ((high & 1) << 3) | (key >> 5),
                modifiers: key & 0x1f,
            },
            0x5200..=0x521f => KeycodeKind::Layer {
                action: LayerAction::To,
                layer,
            },
            0x5220..=0x523f => KeycodeKind::Layer {
                action: LayerAction::Momentary,
                layer,
            },
            0x5240..=0x525f => KeycodeKind::Layer {
                action: LayerAction::Default,
                layer,
            },
            0x5260..=0x527f => KeycodeKind::Layer {
                action: LayerAction::Toggle,
                layer,
            },
            0x5280..=0x529f => KeycodeKind::Layer {
                action: LayerAction::OneShot,
                layer,
            },
            0x52a0..=0x52bf => KeycodeKind::OneShotModifiers(key & 0x1f),
            0x52c0..=0x52df => KeycodeKind::Layer {
                action: LayerAction::TapToggle,
                layer,
            },
            0x52e0..=0x52ff => KeycodeKind::Layer {
                action: LayerAction::PersistentDefault,
                layer,
            },
            _ => KeycodeKind::Raw(self.0),
        }
    }
}
