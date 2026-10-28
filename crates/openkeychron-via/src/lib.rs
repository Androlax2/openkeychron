//! VIA protocol: versions, keymap, layers and macros.
//!
//! Every exchange is one 32-byte request and one 32-byte reply over an
//! [`openkeychron_hid::HidTransport`]. Command and value IDs come from QMK's `quantum/via.h`,
//! as shipped in Keychron's firmware:
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.h>

mod client;
mod command;

pub use client::{Via, ViaError};
pub use command::REPORT_LEN;
