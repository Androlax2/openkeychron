//! Read-only VIA keymap operations driven through replay transports, without hardware.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use openkeychron_hid::{DeviceIdentity, Event, Fixture, ReplayTransport, TransportError};
    use openkeychron_via::{Via, ViaError};

    fn report(prefix: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; 32];
        bytes[..prefix.len()].copy_from_slice(prefix);
        bytes
    }

    fn replay(events: Vec<Event>) -> Via<ReplayTransport> {
        Via::new(ReplayTransport::new(Fixture::new(
            DeviceIdentity {
                name: "synthetic cable keyboard".to_owned(),
                vendor_id: 0x3434,
                product_id: 0x0951,
                usage_page: 0xff60,
                usage: 0x61,
            },
            events,
        )))
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

    #[test]
    fn keymap_reads_layer_count_and_big_endian_codes_from_a_fixture() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic/via-keymap.toml");
        let mut via = Via::new(ReplayTransport::new(Fixture::load(&path).unwrap()));
        assert_eq!(via.layer_count(), Ok(4));
        let keys: Vec<u16> = via.keymap(15).unwrap().into_iter().map(u16::from).collect();
        assert_eq!(
            keys,
            [
                0, 1, 4, 0x00e0, 0x1104, 0x3104, 0x4204, 0x5051, 0x5221, 0x52a1, 0x7e00, 0x7840,
                0xffff, 0x1234, 0x00e7
            ]
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn keymap_uses_exact_chunk_sizes_at_whole_key_boundaries() {
        let cases: &[(usize, &[u8])] = &[
            (1, &[2]),
            (14, &[28]),
            (15, &[28, 2]),
            (28, &[28, 28]),
            (29, &[28, 28, 2]),
        ];
        for &(count, sizes) in cases {
            let mut events = Vec::new();
            let mut offset = 0;
            for &size in sizes {
                let header = [0x12, 0, offset, size];
                let mut reply = report(&header);
                for key in reply[4..4 + usize::from(size)].as_chunks_mut::<2>().0 {
                    key.copy_from_slice(&[0x12, 0x34]);
                }
                events.push(Event::Write {
                    data: report(&header),
                });
                events.push(Event::Read { data: reply });
                offset += size;
            }
            let mut via = replay(events);
            let keys: Vec<u16> = via
                .keymap(count)
                .unwrap()
                .into_iter()
                .map(u16::from)
                .collect();
            assert_eq!(keys, vec![0x1234; count]);
            assert_eq!(via.into_transport().finish(), Ok(()));
        }
    }

    #[test]
    fn keymap_uses_byte_offsets_above_255() {
        let headers = [
            [0x12, 0, 0, 28],
            [0x12, 0, 28, 28],
            [0x12, 0, 56, 28],
            [0x12, 0, 84, 28],
            [0x12, 0, 112, 28],
            [0x12, 0, 140, 28],
            [0x12, 0, 168, 28],
            [0x12, 0, 196, 28],
            [0x12, 0, 224, 28],
            [0x12, 0, 252, 28],
            [0x12, 1, 24, 2],
        ];
        let events = headers
            .into_iter()
            .flat_map(|header| exchange(&header, &header))
            .collect();
        let mut via = replay(events);
        assert_eq!(via.keymap(141).unwrap().len(), 141);
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn keymap_reads_the_largest_addressable_buffer_without_wrapping() {
        let mut events = Vec::new();
        for offset in (0..65_536).step_by(28) {
            let size = u8::try_from((65_536 - offset).min(28)).unwrap();
            let [high, low] = u16::try_from(offset).unwrap().to_be_bytes();
            events.extend(exchange(&[0x12, high, low, size], &[0x12, high, low, size]));
        }
        assert_eq!(
            events[events.len() - 2],
            Event::Write {
                data: report(&[0x12, 0xff, 0xf0, 16])
            }
        );
        let mut via = replay(events);
        assert_eq!(via.keymap(32_768).unwrap().len(), 32_768);
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn keymap_refuses_invalid_sizes_without_sending_anything() {
        for key_count in [0, 32_769, usize::MAX] {
            let mut via = replay(vec![]);
            assert_eq!(
                via.keymap(key_count),
                Err(ViaError::InvalidKeymapSize { key_count })
            );
            assert_eq!(via.into_transport().finish(), Ok(()));
        }
    }

    #[test]
    fn keymap_refuses_a_zero_layer_count() {
        let mut via = replay(exchange(&[0x11], &[0x11, 0]));
        assert_eq!(
            via.layer_count(),
            Err(ViaError::InvalidLayerCount { count: 0 })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn keymap_rejects_a_reply_with_the_wrong_offset_or_size() {
        for (header, actual_offset, actual_size) in [
            ([0x12, 0, 2, 2], 2, 2),
            ([0x12, 1, 0, 2], 256, 2),
            ([0x12, 0, 0, 4], 0, 4),
        ] {
            let mut via = replay(exchange(&[0x12, 0, 0, 2], &header));
            assert_eq!(
                via.keymap(1),
                Err(ViaError::WrongKeymapChunk {
                    expected_offset: 0,
                    actual_offset,
                    expected_size: 2,
                    actual_size,
                })
            );
            assert_eq!(via.into_transport().finish(), Ok(()));
        }
    }

    #[test]
    fn keymap_propagates_protocol_errors() {
        let cases = [
            (report(&[0xff]), ViaError::Unhandled { command: 0x12 }),
            (
                report(&[0x13]),
                ViaError::WrongCommand {
                    expected: 0x12,
                    actual: 0x13,
                },
            ),
            (vec![0x12; 31], ViaError::WrongLength { length: 31 }),
        ];
        for (reply, expected) in cases {
            let mut via = replay(vec![
                Event::Write {
                    data: report(&[0x12, 0, 0, 2]),
                },
                Event::Read { data: reply },
            ]);
            assert_eq!(via.keymap(1), Err(expected));
            assert_eq!(via.into_transport().finish(), Ok(()));
        }
    }

    #[test]
    fn keymap_does_not_return_partial_success_when_a_later_read_times_out() {
        let mut events = exchange(&[0x12, 0, 0, 28], &[0x12, 0, 0, 28]);
        events.extend([
            Event::Write {
                data: report(&[0x12, 0, 28, 2]),
            },
            Event::ReadTimeout,
        ]);
        let mut via = replay(events);
        assert_eq!(
            via.keymap(15),
            Err(ViaError::Transport(TransportError::Timeout))
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }
}
