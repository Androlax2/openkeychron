//! Physical coverage and French display legends through the public layout API.
//!
//! Expected values are hand-checked against Keychron QMK revision
//! `2b493ff95f2665db719b1e95c91babb5036e72f3`:
//! `keyboards/keychron/v5_max/via_json/v5_max_iso_encoder.json` and
//! `quantum/keymap_extras/keymap_french.h` (GPL-2.0-or-later).

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use openkeychron_via::{Legends, Rectangle, V5_MAX_FR_ISO};

    #[test]
    fn layout_lookup_covers_exactly_the_physical_matrix_positions() {
        let columns_by_row: [&[u8]; 6] = [
            &[0, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18],
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 16, 17, 18],
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 16, 17],
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16, 17, 18],
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15, 16, 17, 18],
            &[0, 1, 2, 6, 10, 11, 12, 13, 14, 15, 16, 17],
        ];

        assert_eq!((V5_MAX_FR_ISO.rows, V5_MAX_FR_ISO.columns), (6, 19));
        for row in 0..6 {
            for column in 0..19 {
                let key = V5_MAX_FR_ISO.key(row, column);
                if columns_by_row[usize::from(row)].contains(&column) {
                    assert_eq!(key.map(|key| key.matrix), Some([row, column]));
                } else {
                    assert_eq!(key, None, "unused matrix position ({row}, {column})");
                }
            }
        }
    }

    #[test]
    fn layout_contains_99_unique_reachable_keys_not_114_filled_matrix_slots() {
        assert_eq!(V5_MAX_FR_ISO.keys.len(), 99);
        let mut positions = BTreeSet::new();
        for key in V5_MAX_FR_ISO.keys {
            let [row, column] = key.matrix;
            assert!(
                row < 6 && column < 19,
                "out-of-range position: {:?}",
                key.matrix
            );
            assert!(
                positions.insert(key.matrix),
                "duplicate position: {:?}",
                key.matrix
            );
            assert_eq!(V5_MAX_FR_ISO.key(row, column), Some(key));
        }
    }

    #[test]
    fn layout_lookup_rejects_out_of_range_positions() {
        for (row, column) in [(6, 0), (0, 19), (u8::MAX, 0), (0, u8::MAX)] {
            assert_eq!(V5_MAX_FR_ISO.key(row, column), None);
        }
    }

    #[test]
    fn layout_preserves_both_rectangles_of_iso_enter() {
        let enter = V5_MAX_FR_ISO.key(2, 13).unwrap();
        assert_eq!(
            enter.bounds,
            Rectangle {
                x: 13.75,
                y: 2.25,
                width: 1.25,
                height: 2.0
            }
        );
        assert_eq!(
            enter.secondary_bounds,
            Some(Rectangle {
                x: 13.5,
                y: 2.25,
                width: 1.5,
                height: 1.0
            })
        );
    }

    #[test]
    fn layout_preserves_wide_tall_and_offset_keys() {
        for (row, column, expected) in [
            (
                1,
                13,
                Rectangle {
                    x: 13.0,
                    y: 1.25,
                    width: 2.0,
                    height: 1.0,
                },
            ),
            (
                3,
                18,
                Rectangle {
                    x: 18.5,
                    y: 2.25,
                    width: 1.0,
                    height: 2.0,
                },
            ),
            (
                4,
                18,
                Rectangle {
                    x: 18.5,
                    y: 4.25,
                    width: 1.0,
                    height: 2.0,
                },
            ),
            (
                5,
                6,
                Rectangle {
                    x: 3.75,
                    y: 5.25,
                    width: 6.25,
                    height: 1.0,
                },
            ),
            (
                4,
                14,
                Rectangle {
                    x: 14.25,
                    y: 4.5,
                    width: 1.0,
                    height: 1.0,
                },
            ),
        ] {
            assert_eq!(V5_MAX_FR_ISO.key(row, column).unwrap().bounds, expected);
        }
    }

    #[test]
    fn layout_encoder_press_is_one_matrix_key_not_rotation_keys() {
        let encoders: Vec<_> = V5_MAX_FR_ISO
            .keys
            .iter()
            .filter_map(|key| key.encoder.map(|index| (key.matrix, index)))
            .collect();
        assert_eq!(encoders, [([0, 18], 0)]);
        assert_eq!(V5_MAX_FR_ISO.key(0, 18).unwrap().encoder, Some(0));
    }

    #[test]
    fn layout_displays_french_letters_at_their_physical_positions() {
        for (row, first_column, expected) in
            [(2, 1, "AZERTYUIOP"), (3, 1, "QSDFGHJKLM"), (4, 2, "WXCVBN")]
        {
            let actual: String = (first_column..)
                .zip(expected.chars())
                .map(|(column, _)| V5_MAX_FR_ISO.key(row, column).unwrap().legends.base)
                .collect();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn layout_preserves_french_number_row_shift_and_alt_gr_legends() {
        for (column, base, shift, alt_gr) in [
            (0, "²", None, None),
            (1, "&", Some("1"), None),
            (2, "é", Some("2"), Some("~")),
            (3, "\"", Some("3"), Some("#")),
            (4, "'", Some("4"), Some("{")),
            (5, "(", Some("5"), Some("[")),
            (6, "-", Some("6"), Some("|")),
            (7, "è", Some("7"), Some("`")),
            (8, "_", Some("8"), Some("\\")),
            (9, "ç", Some("9"), None),
            (10, "à", Some("0"), Some("@")),
            (11, ")", Some("°"), Some("]")),
            (12, "=", Some("+"), Some("}")),
        ] {
            assert_eq!(
                V5_MAX_FR_ISO.key(1, column).unwrap().legends,
                Legends {
                    base,
                    shift,
                    alt_gr
                },
                "number-row column {column}"
            );
        }
    }

    #[test]
    fn layout_preserves_french_punctuation_and_euro_legends() {
        for (row, column, base, shift, alt_gr) in [
            (2, 3, "E", None, Some("€")),
            (2, 11, "^", Some("¨"), None),
            (2, 12, "$", Some("£"), Some("¤")),
            (3, 11, "ù", Some("%"), None),
            (3, 12, "*", Some("µ"), None),
            (4, 1, "<", Some(">"), None),
            (4, 8, ",", Some("?"), None),
            (4, 9, ";", Some("."), None),
            (4, 10, ":", Some("/"), None),
            (4, 11, "!", Some("§"), None),
        ] {
            assert_eq!(
                V5_MAX_FR_ISO.key(row, column).unwrap().legends,
                Legends {
                    base,
                    shift,
                    alt_gr
                },
                "matrix position ({row}, {column})"
            );
        }
    }
}
