//! Which physical layout belongs to which keyboard, keyed by USB vendor and product ID.
//!
//! The registry is for reading and display. Knowing a model's layout does not make writes
//! to it safe: which models openkeychron writes to is decided separately, per command.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::{KeyboardLayout, V5_MAX_FR_ISO, ViaDefinition};

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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    #[error(
        "a layout for {vendor_id:04x}:{product_id:04x} is already known; \
         openkeychron does not replace it"
    )]
    AlreadyRegistered { vendor_id: u16, product_id: u16 },
}

/// Layouts by `(vendor_id, product_id)`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceRegistry {
    layouts: BTreeMap<(u16, u16), RegisteredLayout>,
}

impl DeviceRegistry {
    /// The layouts compiled into openkeychron: the V5 Max ISO (`3434:0951`) with French
    /// legends.
    #[must_use]
    pub fn built_in() -> Self {
        Self {
            layouts: BTreeMap::from([(
                (0x3434, 0x0951),
                RegisteredLayout::BuiltIn(&V5_MAX_FR_ISO),
            )]),
        }
    }

    /// Adds a layout under the IDs the definition itself declares. A model that already has
    /// a layout keeps it, so a definition file cannot silently drop built-in legends.
    pub fn register(&mut self, definition: ViaDefinition) -> Result<(), RegistryError> {
        let id = (definition.vendor_id, definition.product_id);
        if self.layouts.contains_key(&id) {
            return Err(RegistryError::AlreadyRegistered {
                vendor_id: id.0,
                product_id: id.1,
            });
        }
        self.layouts
            .insert(id, RegisteredLayout::Definition(definition));
        Ok(())
    }

    #[must_use]
    pub fn layout(&self, vendor_id: u16, product_id: u16) -> Option<&RegisteredLayout> {
        self.layouts.get(&(vendor_id, product_id))
    }
}
