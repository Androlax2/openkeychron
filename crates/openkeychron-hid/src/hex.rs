//! Space-separated lowercase hex (`"01 0c ff"`), the byte encoding used in fixture files.

pub(crate) fn encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn decode(text: &str) -> Result<Vec<u8>, String> {
    text.split_whitespace()
        .map(|pair| {
            if pair.len() != 2 {
                return Err(format!("`{pair}` is not a two-digit hex byte"));
            }
            u8::from_str_radix(pair, 16).map_err(|_| format!("`{pair}` is not a hex byte"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn encodes_as_spaced_lowercase_pairs() {
        assert_eq!(encode(&[0x01, 0x0c, 0xff]), "01 0c ff");
    }

    #[test]
    fn decodes_what_it_encodes() {
        let bytes = [0x00, 0x7f, 0x80, 0xff];
        assert_eq!(decode(&encode(&bytes)), Ok(bytes.to_vec()));
    }

    #[test]
    fn rejects_a_non_hex_pair() {
        assert_eq!(decode("01 zz"), Err("`zz` is not a hex byte".to_owned()));
    }

    #[test]
    fn rejects_an_unpaired_digit() {
        assert_eq!(
            decode("01 2"),
            Err("`2` is not a two-digit hex byte".to_owned())
        );
    }
}
