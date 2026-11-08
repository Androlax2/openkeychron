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

/// The part of a two-part keycode that wraps the other, as QMK's macros build them. Modifier
/// fields use the same five-bit encoding as [`KeycodeKind`]; layers go from 0 to 15.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeycodeWrapper {
    /// `LCTL(…)`: these modifiers held with a basic keycode.
    Modified(u8),
    /// `LCTL_T(…)` or `MT(…)`: these modifiers when held, a basic keycode when tapped.
    ModTap(u8),
    /// `LT(1, …)`: this layer when held, a basic keycode when tapped.
    LayerTap(u8),
    /// `LM(1, …)`: this layer with a modifier field active.
    LayerMod(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeycodeError {
    #[error(
        "modifier field {0:#04x} holds no modifier: expected one or more of Ctrl (0x01), \
         Shift (0x02), Alt (0x04) and GUI (0x08), plus 0x10 for the right-hand ones"
    )]
    InvalidModifiers(u8),
    #[error("layer {0} does not fit a two-part keycode: it holds layers 0 to 15")]
    LayerOutOfRange(u8),
}

// Range starts from `quantum/keycodes.h`; each wrapper's shifts from its macro in
// `quantum/quantum_keycodes.h`: `MT`, `LT` and `LM`, and `LCTL(kc)` as `QK_LCTL | kc`, where
// `QK_LCTL` to `QK_LGUI` are 0x0100 to 0x0800 and `QK_RMODS_MIN` is 0x1000.
const QK_MOD_TAP: u16 = 0x2000;
const QK_LAYER_TAP: u16 = 0x4000;
const QK_LAYER_MOD: u16 = 0x5000;
const LAST_WRAPPED_LAYER: u8 = 15;
/// `MOD_LCTL` to `MOD_LGUI` in `quantum/modifiers.h`; bit 4 only picks the hand.
const CTRL_SHIFT_ALT_GUI: u8 = 0x0f;

impl KeycodeWrapper {
    /// The keycode with `inner` wrapped: a basic keycode, or for [`Self::LayerMod`] a modifier
    /// field.
    ///
    /// # Errors
    ///
    /// When the wrapper or, for [`Self::LayerMod`], `inner` holds no modifier, or its layer is
    /// past 15.
    pub fn wrap(self, inner: u8) -> Result<Keycode, KeycodeError> {
        if let Self::LayerMod(_) = self {
            check_modifiers(inner)?;
        }
        Ok(Keycode(self.bits()? | u16::from(inner)))
    }

    /// The bits this wrapper sets in every keycode it builds: the inner part fills the bits
    /// below them, so a wrapped keycode is `bits | inner`.
    ///
    /// # Errors
    ///
    /// When the wrapper holds no modifier, or its layer is past 15.
    pub fn bits(self) -> Result<u16, KeycodeError> {
        Ok(match self {
            Self::Modified(modifiers) => u16::from(check_modifiers(modifiers)?) << 8,
            Self::ModTap(modifiers) => QK_MOD_TAP | u16::from(check_modifiers(modifiers)?) << 8,
            Self::LayerTap(layer) => QK_LAYER_TAP | u16::from(check_layer(layer)?) << 8,
            Self::LayerMod(layer) => QK_LAYER_MOD | u16::from(check_layer(layer)?) << 5,
        })
    }
}

pub(crate) fn check_modifiers(modifiers: u8) -> Result<u8, KeycodeError> {
    if modifiers > 0x1f || modifiers & CTRL_SHIFT_ALT_GUI == 0 {
        return Err(KeycodeError::InvalidModifiers(modifiers));
    }
    Ok(modifiers)
}

fn check_layer(layer: u8) -> Result<u8, KeycodeError> {
    if layer > LAST_WRAPPED_LAYER {
        return Err(KeycodeError::LayerOutOfRange(layer));
    }
    Ok(layer)
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
