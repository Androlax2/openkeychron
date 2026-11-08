//! Mock keyboard files: what they hold, and what is refused.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use openkeychron_hid::DeviceIdentity;
    use openkeychron_mock::MockKeyboard;

    fn synthetic(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic")
            .join(name)
    }

    /// The synthetic mock keyboard file with `from` replaced by `to`, written to a fresh file.
    fn edited(test: &str, from: &str, to: &str) -> PathBuf {
        let original = std::fs::read_to_string(synthetic("v5-max-iso.mock.toml")).unwrap();
        assert!(original.contains(from), "{from:?} is not in the file");
        let path = std::env::temp_dir().join(format!(
            "openkeychron-mock-{test}-{}.mock.toml",
            std::process::id()
        ));
        std::fs::write(&path, original.replace(from, to)).unwrap();
        path
    }

    #[test]
    fn loads_the_identity_the_keyboard_is_listed_under() {
        let keyboard = MockKeyboard::load(&synthetic("v5-max-iso.mock.toml")).unwrap();
        assert_eq!(
            keyboard.identity(),
            &DeviceIdentity {
                name: "mock V5 Max ISO".to_owned(),
                vendor_id: 0x3434,
                product_id: 0x0951,
                usage_page: 0xff60,
                usage: 0x61,
            }
        );
    }

    #[test]
    fn refuses_another_format_version() {
        let path = edited("version", "format_version = 1", "format_version = 2");
        let error = MockKeyboard::load(&path).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            error.to_string(),
            "mock keyboard format_version 2 is not supported (this build reads 1)"
        );
    }

    #[test]
    fn refuses_a_model_without_a_built_in_layout() {
        let path = edited("layout", "product_id = 2385", "product_id = 2384");
        let error = MockKeyboard::load(&path).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            error.to_string(),
            "this build has no keyboard layout for 3434:0950, so a mock keyboard cannot size its \
             keymap"
        );
    }

    #[test]
    fn refuses_a_key_the_format_does_not_have() {
        let path = edited("unknown", "usage = 97", "usage = 97\nserial = \"1234\"");
        let error = MockKeyboard::load(&path).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert!(
            error.to_string().starts_with("invalid mock keyboard: "),
            "{error}"
        );
        assert!(
            error.to_string().contains("unknown field `serial`"),
            "{error}"
        );
    }

    #[test]
    fn refuses_a_file_it_cannot_read_and_says_why() {
        let missing = synthetic("no-such.mock.toml");
        let error = MockKeyboard::load(&missing).unwrap_err();
        assert_eq!(
            error.to_string(),
            "cannot read the mock keyboard: No such file or directory (os error 2)"
        );
    }
}
