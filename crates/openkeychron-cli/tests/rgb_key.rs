//! Runs `openkeychron rgb key` (per-key RGB) against fixtures.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic")
            .join(name)
    }

    fn backup_path(test: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "openkeychron-rgb-key-{test}-{}.json",
            std::process::id()
        ))
    }

    fn rgb_key(fixture_name: &str, backup: &Path, key: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_openkeychron"))
            .args([
                "rgb",
                "key",
                "--backup",
                backup.to_str().unwrap(),
                "--fixture",
                fixture(fixture_name).to_str().unwrap(),
                key,
                "0",
                "255",
            ])
            .output()
            .unwrap()
    }

    fn stdout(output: &Output) -> String {
        String::from_utf8(output.stdout.clone()).unwrap()
    }

    fn stderr(output: &Output) -> String {
        String::from_utf8(output.stderr.clone()).unwrap()
    }

    #[test]
    fn perkey_sets_a_key_colour_and_saves_the_previous_one_to_the_backup() {
        let backup = backup_path("set");
        let output = rgb_key("cli-rgb-key.toml", &backup, "A");
        let saved = std::fs::read_to_string(&backup);
        std::fs::remove_file(&backup).ok();
        assert!(output.status.success(), "{}", stderr(&output));
        let saved: serde_json::Value = serde_json::from_str(&saved.unwrap()).unwrap();
        assert_eq!(
            saved,
            serde_json::json!({
                "key": "A",
                "row": 2,
                "column": 1,
                "led": 38,
                "hue": 170,
                "saturation": 255,
                "value": 255,
            })
        );
    }

    #[test]
    fn perkey_prints_the_restore_command() {
        let backup = backup_path("restore");
        let output = rgb_key("cli-rgb-key.toml", &backup, "a");
        std::fs::remove_file(&backup).ok();
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(
            stdout(&output).contains("openkeychron rgb key --backup <FILE> 2,1 170 255"),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn perkey_says_the_firmware_lacks_per_key_rgb_and_writes_nothing() {
        let backup = backup_path("unsupported");
        let output = rgb_key("cli-rgb-key-unsupported.toml", &backup, "A");
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains(
                "does not report support for per-key RGB (Keychron firmware 1.1.0 or later)"
            ),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }

    #[test]
    fn perkey_refuses_to_write_through_a_receiver() {
        let backup = backup_path("receiver");
        let output = rgb_key("cli-info.toml", &backup, "A");
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("only written over a USB cable"),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }

    #[test]
    fn perkey_asks_for_a_matrix_position_when_a_legend_names_several_keys() {
        let backup = backup_path("ambiguous");
        let output = rgb_key("cli-rgb-key.toml", &backup, "Shift");
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("`Shift` names several keys (4,0, 4,12)"),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }

    #[test]
    fn perkey_sets_the_previous_colour_again_and_saves_nothing_when_the_read_back_differs() {
        let backup = backup_path("mismatch");
        let output = rgb_key("cli-rgb-key-mismatch.toml", &backup, "A");
        std::fs::remove_file(&backup).ok();
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("the previous colour is set again and nothing was saved"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn perkey_says_a_key_without_an_led_has_none_and_writes_nothing() {
        let backup = backup_path("no-led");
        let output = rgb_key("cli-rgb-key-no-led.toml", &backup, "A");
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("key `A` (2,1) has no LED"),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }
}
