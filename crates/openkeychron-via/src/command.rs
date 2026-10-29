//! VIA command and value IDs, from `enum via_command_id` and `enum via_keyboard_value_id` in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.h>.
//! Only the commands implemented so far are listed.

/// Size of every VIA request and reply (`RAW_EPSIZE`).
pub const REPORT_LEN: usize = 32;

/// `id_unhandled`: the firmware puts this in byte 0 of the reply when it does not know the
/// command or value ID.
pub(crate) const UNHANDLED: u8 = 0xFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    GetProtocolVersion,
    GetKeyboardValue,
    CustomSetValue,
    CustomGetValue,
    CustomSave,
    GetLayerCount,
    GetKeymapBuffer,
}

impl Command {
    pub(crate) const fn id(self) -> u8 {
        match self {
            Self::GetProtocolVersion => 0x01,
            Self::GetKeyboardValue => 0x02,
            Self::CustomSetValue => 0x07,
            Self::CustomGetValue => 0x08,
            Self::CustomSave => 0x09,
            Self::GetLayerCount => 0x11,
            Self::GetKeymapBuffer => 0x12,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyboardValue {
    Uptime,
    LayoutOptions,
    FirmwareVersion,
}

impl KeyboardValue {
    pub(crate) const fn id(self) -> u8 {
        match self {
            Self::Uptime => 0x01,
            Self::LayoutOptions => 0x02,
            Self::FirmwareVersion => 0x04,
        }
    }
}

/// A request report: the command ID, its arguments, then zeros up to [`REPORT_LEN`].
pub(crate) fn request(command: Command, arguments: &[u8]) -> [u8; REPORT_LEN] {
    let mut report = [0u8; REPORT_LEN];
    report[0] = command.id();
    report[1..=arguments.len()].copy_from_slice(arguments);
    report
}

#[cfg(test)]
mod tests {
    use super::{Command, KeyboardValue, REPORT_LEN, request};

    #[test]
    fn a_request_is_the_command_id_then_its_arguments_then_zeros() {
        let report = request(Command::GetKeyboardValue, &[KeyboardValue::Uptime.id()]);
        let mut expected = [0u8; REPORT_LEN];
        expected[0] = 0x02;
        expected[1] = 0x01;
        assert_eq!(report, expected);
    }
}
