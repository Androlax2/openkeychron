//! Public keymap protocol operations against hand-written synthetic replay sessions.

#[cfg(test)]
mod tests {
    use openkeychron_hid::{DeviceIdentity, Event, Fixture, ReplayTransport, TransportError};
    use openkeychron_via::{EncoderDirection, Keycode, Via, ViaError};

    fn report(prefix: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; 32];
        bytes[..prefix.len()].copy_from_slice(prefix);
        bytes
    }

    fn exchange(request: &[u8], reply: &[u8]) -> Vec<Event> {
        vec![
            Event::Write {
                data: report(request),
            },
            Event::Read {
                data: report(reply),
            },
        ]
    }

    fn replay(events: Vec<Event>) -> Via<ReplayTransport> {
        Via::new(ReplayTransport::new(Fixture::new(
            DeviceIdentity {
                name: "synthetic V5 Max ISO write verification".to_owned(),
                vendor_id: 0x3434,
                product_id: 0x0951,
                usage_page: 0xff60,
                usage: 0x61,
            },
            events,
        )))
    }

    #[test]
    fn write_keycode_verifies_actual_state_after_the_setter_echo() {
        let mut events = exchange(&[5, 2, 2, 1, 0x12, 0x34], &[5, 2, 2, 1, 0x12, 0x34]);
        events.extend(exchange(&[4, 2, 2, 1], &[4, 2, 2, 1, 0x12, 0x34]));
        let mut via = replay(events);
        assert_eq!(via.write_keycode(2, 2, 1, Keycode::from(0x1234)), Ok(()));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn write_keycode_does_not_treat_the_setter_echo_as_read_back() {
        let mut events = exchange(&[5, 2, 2, 1, 0x12, 0x34], &[5, 2, 2, 1, 0x12, 0x34]);
        events.extend(exchange(&[4, 2, 2, 1], &[4, 2, 2, 1, 0x56, 0x78]));
        let mut via = replay(events);
        assert_eq!(
            via.write_keycode(2, 2, 1, Keycode::from(0x1234)),
            Err(ViaError::KeycodeWriteMismatch {
                command: 5,
                address: [2, 2, 1],
                written: 0x1234,
                read: 0x5678,
            })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn write_keycode_rejects_a_read_back_from_another_address() {
        let mut events = exchange(&[5, 2, 2, 1, 0x12, 0x34], &[5, 2, 2, 1, 0x12, 0x34]);
        events.extend(exchange(&[4, 2, 2, 1], &[4, 2, 2, 2, 0x12, 0x34]));
        let mut via = replay(events);
        assert_eq!(
            via.write_keycode(2, 2, 1, Keycode::from(0x1234)),
            Err(ViaError::WrongKeymapAddress {
                command: 4,
                expected: [2, 2, 1],
                actual: [2, 2, 2],
            })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn write_encoder_support_reads_both_wire_directions_without_swapping_them() {
        let mut events = exchange(&[0x14, 1, 0, 0], &[0x14, 1, 0, 0, 0xab, 0xcd]);
        events.extend(exchange(&[0x14, 1, 0, 1], &[0x14, 1, 0, 1, 0x12, 0x34]));
        let mut via = replay(events);
        assert_eq!(
            via.encoder_keycode(1, 0, EncoderDirection::CounterClockwise),
            Ok(Keycode::from(0xabcd))
        );
        assert_eq!(
            via.encoder_keycode(1, 0, EncoderDirection::Clockwise),
            Ok(Keycode::from(0x1234))
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn write_encoder_verifies_each_direction_after_the_setter_echo() {
        for (direction, flag) in [
            (EncoderDirection::CounterClockwise, 0),
            (EncoderDirection::Clockwise, 1),
        ] {
            let mut events = exchange(
                &[0x15, 1, 0, flag, 0xab, 0xcd],
                &[0x15, 1, 0, flag, 0xab, 0xcd],
            );
            events.extend(exchange(
                &[0x14, 1, 0, flag],
                &[0x14, 1, 0, flag, 0xab, 0xcd],
            ));
            let mut via = replay(events);
            assert_eq!(
                via.write_encoder_keycode(1, 0, direction, Keycode::from(0xabcd)),
                Ok(())
            );
            assert_eq!(via.into_transport().finish(), Ok(()));
        }
    }

    #[test]
    fn write_encoder_reports_a_mismatched_actual_assignment() {
        let mut events = exchange(&[0x15, 1, 0, 1, 0xab, 0xcd], &[0x15, 1, 0, 1, 0xab, 0xcd]);
        events.extend(exchange(&[0x14, 1, 0, 1], &[0x14, 1, 0, 1, 0x12, 0x34]));
        let mut via = replay(events);
        assert_eq!(
            via.write_encoder_keycode(1, 0, EncoderDirection::Clockwise, Keycode::from(0xabcd)),
            Err(ViaError::KeycodeWriteMismatch {
                command: 0x15,
                address: [1, 0, 1],
                written: 0xabcd,
                read: 0x1234,
            })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn write_encoder_rejects_a_reply_for_the_other_direction() {
        let mut via = replay(exchange(&[0x14, 1, 0, 0], &[0x14, 1, 0, 1, 0x12, 0x34]));
        assert_eq!(
            via.encoder_keycode(1, 0, EncoderDirection::CounterClockwise),
            Err(ViaError::WrongKeymapAddress {
                command: 0x14,
                expected: [1, 0, 0],
                actual: [1, 0, 1],
            })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    fn first_buffer_chunk() -> Vec<Event> {
        let mut write = vec![0x13, 0, 0, 28];
        write.extend([0x12, 0x34].repeat(14));
        let mut read = write.clone();
        read[0] = 0x12;
        let mut events = exchange(&write, &write);
        events.extend(exchange(&[0x12, 0, 0, 28], &read));
        events
    }

    #[test]
    fn write_buffer_reads_back_each_chunk_and_uses_an_exact_short_final_chunk() {
        for count in [14, 15] {
            let mut events = first_buffer_chunk();
            let mut keys = vec![Keycode::from(0x1234); 14];
            if count == 15 {
                events.extend(exchange(
                    &[0x13, 0, 28, 2, 0x56, 0x78],
                    &[0x13, 0, 28, 2, 0x56, 0x78],
                ));
                events.extend(exchange(&[0x12, 0, 28, 2], &[0x12, 0, 28, 2, 0x56, 0x78]));
                keys.push(Keycode::from(0x5678));
            }
            let mut via = replay(events);
            assert_eq!(via.write_keymap(&keys), Ok(()));
            assert_eq!(via.into_transport().finish(), Ok(()));
        }
    }

    #[test]
    fn write_buffer_reports_a_later_chunk_mismatch_without_claiming_success() {
        let mut events = first_buffer_chunk();
        events.extend(exchange(
            &[0x13, 0, 28, 2, 0x56, 0x78],
            &[0x13, 0, 28, 2, 0x56, 0x78],
        ));
        events.extend(exchange(&[0x12, 0, 28, 2], &[0x12, 0, 28, 2, 0xff, 0xff]));
        let mut keys = vec![Keycode::from(0x1234); 14];
        keys.push(Keycode::from(0x5678));
        let mut via = replay(events);
        assert_eq!(
            via.write_keymap(&keys),
            Err(ViaError::KeymapWriteMismatch {
                offset: 28,
                written: 0x5678,
                read: 0xffff
            })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn write_buffer_rejects_invalid_sizes_without_sending_reports() {
        for count in [0, 32_769] {
            let mut via = replay(vec![]);
            assert_eq!(
                via.write_keymap(&vec![Keycode::from(0); count]),
                Err(ViaError::InvalidKeymapSize { key_count: count })
            );
            assert_eq!(via.into_transport().finish(), Ok(()));
        }
    }

    #[test]
    fn write_buffer_rejects_forged_offsets_on_write_and_read_replies() {
        for forged_write in [false, true] {
            let mut events = exchange(
                &[0x13, 0, 0, 2, 0x12, 0x34],
                &[0x13, 0, u8::from(forged_write) * 2, 2, 0x12, 0x34],
            );
            if !forged_write {
                events.extend(exchange(&[0x12, 0, 0, 2], &[0x12, 0, 2, 2, 0x12, 0x34]));
            }
            let mut via = replay(events);
            assert_eq!(
                via.write_keymap(&[Keycode::from(0x1234)]),
                Err(ViaError::WrongKeymapChunk {
                    expected_offset: 0,
                    actual_offset: 2,
                    expected_size: 2,
                    actual_size: 2,
                })
            );
            assert_eq!(via.into_transport().finish(), Ok(()));
        }
    }

    #[test]
    fn write_keycode_propagates_a_timeout_after_the_write() {
        let mut events = exchange(&[5, 0, 2, 1, 0x12, 0x34], &[5, 0, 2, 1, 0x12, 0x34]);
        events.extend([
            Event::Write {
                data: report(&[4, 0, 2, 1]),
            },
            Event::ReadTimeout,
        ]);
        let mut via = replay(events);
        assert_eq!(
            via.write_keycode(0, 2, 1, Keycode::from(0x1234)),
            Err(ViaError::Transport(TransportError::Timeout))
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reset_keymap_before_write_sends_dynamic_keymap_reset_and_accepts_its_echo() {
        let mut via = replay(exchange(&[6], &[6]));
        assert_eq!(via.reset_keymap(), Ok(()));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reset_keymap_before_write_reports_firmware_without_the_command() {
        let mut via = replay(exchange(&[6], &[0xff]));
        assert_eq!(via.reset_keymap(), Err(ViaError::Unhandled { command: 6 }));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }
}
