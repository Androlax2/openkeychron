//! Macro buffer commands against hand-written synthetic replay sessions.

#[cfg(test)]
mod tests {
    use openkeychron_hid::{DeviceIdentity, Event, Fixture, ReplayTransport};
    use openkeychron_via::{Via, ViaError};

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

    /// A setter whose echo matches, then the getter that reads the same bytes back.
    fn verified_write(offset: u16, bytes: &[u8], read_back: &[u8]) -> Vec<Event> {
        let [high, low] = offset.to_be_bytes();
        let size = u8::try_from(bytes.len()).unwrap();
        let set = [&[0x0f, high, low, size][..], bytes].concat();
        let mut events = exchange(&set, &set);
        events.extend(exchange(
            &[0x0e, high, low, size],
            &[&[0x0e, high, low, size][..], read_back].concat(),
        ));
        events
    }

    fn replay(events: Vec<Event>) -> Via<ReplayTransport> {
        Via::new(ReplayTransport::new(Fixture::new(
            DeviceIdentity {
                name: "synthetic macro buffer".to_owned(),
                vendor_id: 0x3434,
                product_id: 0x0951,
                usage_page: 0xff60,
                usage: 0x61,
            },
            events,
        )))
    }

    /// The size query `write_macro_buffer` sends before its first write.
    fn size_query(size: u16) -> Vec<Event> {
        let [high, low] = size.to_be_bytes();
        exchange(&[0x0d], &[0x0d, high, low])
    }

    fn sample_buffer() -> Vec<u8> {
        let mut buffer = b"hello\0\x01\x01\x28\0".to_vec();
        buffer.resize(40, 0);
        buffer
    }

    #[test]
    fn reads_the_macro_count() {
        let mut via = replay(exchange(&[0x0c], &[0x0c, 16]));
        assert_eq!(via.macro_count(), Ok(16));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reads_the_macro_buffer_size_as_big_endian() {
        let mut via = replay(exchange(&[0x0d], &[0x0d, 0x07, 0x9a]));
        assert_eq!(via.macro_buffer_size(), Ok(0x079a));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rejects_a_zero_macro_buffer_size() {
        let mut via = replay(exchange(&[0x0d], &[0x0d, 0, 0]));
        assert_eq!(via.macro_buffer_size(), Err(ViaError::EmptyMacroBuffer));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reads_the_macro_buffer_in_chunks_of_28_bytes() {
        let buffer = sample_buffer();
        let mut events = exchange(
            &[0x0e, 0, 0, 28],
            &[&[0x0e, 0, 0, 28][..], &buffer[..28]].concat(),
        );
        events.extend(exchange(
            &[0x0e, 0, 28, 12],
            &[&[0x0e, 0, 28, 12][..], &buffer[28..]].concat(),
        ));
        let mut via = replay(events);
        assert_eq!(via.macro_buffer(40), Ok(buffer));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rejects_a_macro_chunk_reply_for_another_offset() {
        let mut via = replay(exchange(&[0x0e, 0, 0, 4], &[0x0e, 0, 1, 4]));
        assert_eq!(
            via.macro_buffer(4),
            Err(ViaError::WrongMacroChunk {
                command: 0x0e,
                expected_offset: 0,
                actual_offset: 1,
                expected_size: 4,
                actual_size: 4,
            })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn writes_the_macro_buffer_between_setting_and_clearing_the_last_byte_flag() {
        let buffer = sample_buffer();
        let mut events = size_query(40);
        events.extend(verified_write(39, &[0xff], &[0xff]));
        events.extend(verified_write(0, &buffer[..28], &buffer[..28]));
        events.extend(verified_write(28, &buffer[28..39], &buffer[28..39]));
        events.extend(verified_write(39, &[0], &[0]));
        let mut via = replay(events);
        assert_eq!(via.write_macro_buffer(&buffer), Ok(()));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn macro_write_fails_when_a_chunk_reads_back_different() {
        let buffer = sample_buffer();
        let mut read_back = buffer[..28].to_vec();
        read_back[1] = b'a';
        let mut events = size_query(40);
        events.extend(verified_write(39, &[0xff], &[0xff]));
        events.extend(verified_write(0, &buffer[..28], &read_back));
        let mut via = replay(events);
        assert_eq!(
            via.write_macro_buffer(&buffer),
            Err(ViaError::MacroWriteMismatch {
                offset: 1,
                written: b'e',
                read: b'a',
            })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn macro_write_fails_when_the_flag_does_not_read_back() {
        let mut events = size_query(40);
        events.extend(verified_write(39, &[0xff], &[0]));
        let mut via = replay(events);
        assert_eq!(
            via.write_macro_buffer(&sample_buffer()),
            Err(ViaError::MacroWriteMismatch {
                offset: 39,
                written: 0xff,
                read: 0,
            })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn macro_write_refuses_a_buffer_of_another_size_before_writing() {
        let mut via = replay(size_query(41));
        assert_eq!(
            via.write_macro_buffer(&sample_buffer()),
            Err(ViaError::MacroBufferSizeMismatch {
                keyboard: 41,
                buffer: 40,
            })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn macro_write_rejects_a_buffer_that_would_leave_macros_disabled() {
        let mut buffer = sample_buffer();
        buffer[39] = 1;
        let mut via = replay(vec![]);
        assert_eq!(
            via.write_macro_buffer(&buffer),
            Err(ViaError::InvalidMacroBuffer { length: 40 })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn macro_write_rejects_an_empty_buffer() {
        let mut via = replay(vec![]);
        assert_eq!(
            via.write_macro_buffer(&[]),
            Err(ViaError::InvalidMacroBuffer { length: 0 })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }
}
