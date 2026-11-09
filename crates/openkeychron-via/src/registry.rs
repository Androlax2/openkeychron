//! Which physical layout belongs to which keyboard, keyed by USB vendor and product ID.
//!
//! Knowing a model's layout does not make writes to it safe. Each entry says on its own
//! whether writes were verified on hardware ([`WriteAccess`]); a definition file registered at
//! run time is always read-only (`docs/DECISIONS.md`, #41 and #91).

use std::collections::BTreeMap;

use thiserror::Error;

use crate::keycode_name::{V5_MAX_KEYCHRON_NAMES, keychron_name, keychron_names_listed};
use crate::{KeyboardLayout, Keycode, V5_MAX_FR_ISO, ViaDefinition};

/// A model's layout: a table compiled into openkeychron, or one read from a VIA definition.
#[derive(Debug, Clone, PartialEq)]
pub enum RegisteredLayout {
    /// Built in, with display legends.
    BuiltIn(&'static KeyboardLayout),
    /// Read from a VIA definition file, which has no legends.
    Definition(ViaDefinition),
}

impl RegisteredLayout {
    #[must_use]
    pub fn rows(&self) -> u8 {
        match self {
            Self::BuiltIn(layout) => layout.rows,
            Self::Definition(definition) => definition.rows,
        }
    }

    #[must_use]
    pub fn columns(&self) -> u8 {
        match self {
            Self::BuiltIn(layout) => layout.columns,
            Self::Definition(definition) => definition.columns,
        }
    }

    /// Whether a physical key sits at this matrix position.
    #[must_use]
    pub fn has_key(&self, row: u8, column: u8) -> bool {
        match self {
            Self::BuiltIn(layout) => layout.key(row, column).is_some(),
            Self::Definition(definition) => definition.key(row, column).is_some(),
        }
    }

    /// The physical base legend at this position, when the layout has legends.
    #[must_use]
    pub fn legend(&self, row: u8, column: u8) -> Option<&str> {
        match self {
            Self::BuiltIn(layout) => layout.key(row, column).map(|key| key.legends.base),
            Self::Definition(_) => None,
        }
    }
}

/// Whether openkeychron writes a model's keymap, macros and lighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAccess {
    /// Writes were verified on the real keyboard over a USB cable. A receiver stays refused:
    /// it does not identify the keyboard behind it.
    VerifiedOverCable,
    /// No write was verified on hardware: reading and display only.
    ReadOnly,
}

/// One known model: its name, its layout, whether it may be written and the names of its
/// Keychron custom keycodes.
#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredModel {
    name: String,
    layout: RegisteredLayout,
    write_access: WriteAccess,
    /// From `QK_KB_0` up, as this model's firmware source settles them; empty when it does not.
    keychron_names: &'static [&'static str],
}

impl RegisteredModel {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn layout(&self) -> &RegisteredLayout {
        &self.layout
    }

    #[must_use]
    pub fn write_access(&self) -> WriteAccess {
        self.write_access
    }

    /// [`Keycode::name`], or this model's name for a Keychron custom keycode when its firmware
    /// source settles it. A model read from a definition file names none.
    #[must_use]
    pub fn keycode_name(&self, keycode: Keycode) -> Option<String> {
        keycode
            .name()
            .or_else(|| keychron_name(keycode, self.keychron_names).map(str::to_owned))
    }

    /// Every Keychron custom keycode this model names, by value, in keycode order. Empty for a
    /// model read from a definition file.
    #[must_use]
    pub fn custom_keycode_names(&self) -> Vec<(u16, String)> {
        keychron_names_listed(self.keychron_names)
            .into_iter()
            .map(|(keycode, name)| (keycode, name.to_owned()))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    #[error(
        "a layout for {vendor_id:04x}:{product_id:04x} is already known; \
         openkeychron does not replace it"
    )]
    AlreadyRegistered { vendor_id: u16, product_id: u16 },
}

/// Models by `(vendor_id, product_id)`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceRegistry {
    models: BTreeMap<(u16, u16), RegisteredModel>,
}

impl DeviceRegistry {
    /// The models compiled into openkeychron: the V5 Max ISO (`3434:0951`) with French
    /// legends, the only model whose writes were verified on hardware (over a USB cable).
    #[must_use]
    pub fn built_in() -> Self {
        Self {
            models: BTreeMap::from([(
                (0x3434, 0x0951),
                // Marking another model verified needs #99 first: the write paths still use
                // the V5 Max ISO geometry.
                RegisteredModel {
                    name: "V5 Max ISO".to_owned(),
                    layout: RegisteredLayout::BuiltIn(&V5_MAX_FR_ISO),
                    write_access: WriteAccess::VerifiedOverCable,
                    keychron_names: V5_MAX_KEYCHRON_NAMES,
                },
            )]),
        }
    }

    /// Adds a read-only model under the IDs the definition itself declares: a file the user
    /// passes never enables writes. A model that is already known keeps its entry, so a
    /// definition file cannot silently drop built-in legends or write access.
    pub fn register(&mut self, definition: ViaDefinition) -> Result<(), RegistryError> {
        let id = (definition.vendor_id, definition.product_id);
        if self.models.contains_key(&id) {
            return Err(RegistryError::AlreadyRegistered {
                vendor_id: id.0,
                product_id: id.1,
            });
        }
        self.models.insert(
            id,
            RegisteredModel {
                name: definition.name.clone(),
                layout: RegisteredLayout::Definition(definition),
                write_access: WriteAccess::ReadOnly,
                // A definition's customKeycodes are Launcher labels, not firmware names.
                keychron_names: &[],
            },
        );
        Ok(())
    }

    #[must_use]
    pub fn model(&self, vendor_id: u16, product_id: u16) -> Option<&RegisteredModel> {
        self.models.get(&(vendor_id, product_id))
    }

    #[must_use]
    pub fn layout(&self, vendor_id: u16, product_id: u16) -> Option<&RegisteredLayout> {
        self.model(vendor_id, product_id)
            .map(RegisteredModel::layout)
    }

    /// The models whose writes were verified on hardware, by `(vendor_id, product_id)`.
    pub fn write_verified_models(&self) -> impl Iterator<Item = ((u16, u16), &RegisteredModel)> {
        self.models
            .iter()
            .filter(|(_, model)| model.write_access == WriteAccess::VerifiedOverCable)
            .map(|(id, model)| (*id, model))
    }
}
