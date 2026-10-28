//! Runs the `openkeychron` binary against fixtures, the way a user or script would.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::{Command, Output};

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic")
            .join(name)
    }

    fn openkeychron(arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_openkeychron"))
            .args(arguments)
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
    fn info_prints_the_via_values_from_a_fixture() {
        let path = fixture("cli-info.toml");
        let output = openkeychron(&["info", "--fixture", path.to_str().unwrap()]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains("VIA protocol:         0x000c"), "{text}");
        assert!(text.contains("Uptime:               12.345 s"), "{text}");
        assert!(
            text.contains("Keychron Link (3434:d030, receiver)"),
            "{text}"
        );
        assert!(
            text.contains("Keychron firmware:    v1.1.0 (built 2025-03-17-10:00:00)"),
            "{text}"
        );
    }

    #[test]
    fn info_says_why_the_keychron_firmware_version_is_not_reported() {
        let path = fixture("cli-info-no-keychron.toml");
        let output = openkeychron(&["info", "--fixture", path.to_str().unwrap()]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(
            text.contains(
                "Keychron firmware:    not reported (the firmware does not answer Keychron command 0xa0)"
            ),
            "{text}"
        );
    }

    #[test]
    fn info_json_reports_a_missing_keychron_version_as_null() {
        let path = fixture("cli-info-no-keychron.toml");
        let output = openkeychron(&["info", "--json", "--fixture", path.to_str().unwrap()]);
        assert!(output.status.success(), "{}", stderr(&output));
        let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert!(json["keychron_firmware_version"].is_null());
        assert!(json["keychron_firmware_build_date"].is_null());
        assert!(json["keychron_firmware_not_reported"].is_string());
    }

    #[test]
    fn info_json_is_machine_readable() {
        let path = fixture("cli-info.toml");
        let output = openkeychron(&["info", "--json", "--fixture", path.to_str().unwrap()]);
        assert!(output.status.success(), "{}", stderr(&output));
        let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["via_protocol_version"], 12);
        assert_eq!(json["uptime_ms"], 12_345);
        assert_eq!(json["device"]["product_id"], 0xd030);
        assert_eq!(json["device"]["is_receiver"], true);
        assert_eq!(json["keychron_firmware_version"], "v1.1.0");
        assert_eq!(json["keychron_firmware_build_date"], "2025-03-17-10:00:00");
        assert!(json["keychron_firmware_not_reported"].is_null());
    }

    #[test]
    fn info_fails_when_the_session_differs_from_the_fixture() {
        let path = fixture("via-keyboard-values.toml");
        let output = openkeychron(&["info", "--fixture", path.to_str().unwrap()]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("replay: write #0"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn info_fails_when_the_fixture_has_events_it_never_reached() {
        let longer = std::fs::read_to_string(fixture("cli-info.toml")).unwrap()
            + "\n[[event]]\nkind = \"read_timeout\"\n";
        let path = std::env::temp_dir().join(format!("openkeychron-{}.toml", std::process::id()));
        std::fs::write(&path, longer).unwrap();
        let output = openkeychron(&["info", "--fixture", path.to_str().unwrap()]);
        std::fs::remove_file(&path).unwrap();
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("1 recorded events were never reached"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn list_shows_the_device_a_fixture_was_recorded_from() {
        let path = fixture("cli-info.toml");
        let output = openkeychron(&["list", "--json", "--fixture", path.to_str().unwrap()]);
        assert!(output.status.success(), "{}", stderr(&output));
        let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json[0]["vendor_id"], 0x3434);
        assert_eq!(json[0]["is_receiver"], true);
    }

    #[test]
    fn fixture_and_record_cannot_be_combined() {
        let path = fixture("cli-info.toml");
        let path = path.to_str().unwrap();
        let output = openkeychron(&["info", "--fixture", path, "--record", "/tmp/unused.toml"]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("cannot be used with"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn list_refuses_record() {
        let output = openkeychron(&["list", "--record", "/tmp/unused.toml"]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("--record applies to"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn settings_get_prints_debounce_and_active_snap_click_pairs() {
        let path = fixture("cli-settings-get.toml");
        let output = openkeychron(&["settings", "get", "--fixture", path.to_str().unwrap()]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(
            text.contains("Debounce:   50 ms, symmetric-defer-global"),
            "{text}"
        );
        assert!(
            text.contains("pair 0: last-input on keys 0x04 and 0x07"),
            "{text}"
        );
        assert!(
            text.contains("Wireless:   backlight off after 600 s, sleep after 7200 s"),
            "{text}"
        );
    }

    #[test]
    fn settings_set_wireless_saves_a_backup_and_reads_the_values_back() {
        let path = fixture("cli-settings-set-wireless.toml");
        let backup = std::env::temp_dir().join(format!(
            "openkeychron-wireless-backup-{}.json",
            std::process::id()
        ));
        let output = openkeychron(&[
            "settings",
            "set",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "wireless",
            "30",
            "3600",
        ]);
        let saved = std::fs::read_to_string(&backup);
        std::fs::remove_file(&backup).ok();
        assert!(output.status.success(), "{}", stderr(&output));
        let saved: serde_json::Value = serde_json::from_str(&saved.unwrap()).unwrap();
        assert_eq!(saved["wireless"]["backlight_off_seconds"], 600);
        assert_eq!(saved["wireless"]["sleep_seconds"], 7200);
        assert!(
            stdout(&output).contains("wireless 600 7200"),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn settings_set_debounce_saves_a_backup_and_reads_the_value_back() {
        let path = fixture("cli-settings-set-debounce.toml");
        let backup =
            std::env::temp_dir().join(format!("openkeychron-backup-{}.json", std::process::id()));
        let output = openkeychron(&[
            "settings",
            "set",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "debounce",
            "symmetric-eager-per-key",
            "10",
        ]);
        let saved = std::fs::read_to_string(&backup);
        std::fs::remove_file(&backup).ok();
        assert!(output.status.success(), "{}", stderr(&output));
        let saved: serde_json::Value = serde_json::from_str(&saved.unwrap()).unwrap();
        assert_eq!(saved["debounce"]["algorithm"], "symmetric-defer-global");
        assert_eq!(saved["debounce"]["time_ms"], 50);
        assert!(
            stdout(&output).contains("debounce symmetric-defer-global 50"),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn settings_set_refuses_to_write_through_a_receiver() {
        let path = fixture("cli-info.toml");
        let backup =
            std::env::temp_dir().join(format!("openkeychron-unused-{}.json", std::process::id()));
        let output = openkeychron(&[
            "settings",
            "set",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "debounce",
            "none",
            "0",
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("only written over a USB cable"),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }

    #[test]
    fn settings_set_requires_a_backup_file() {
        let path = fixture("cli-settings-set-debounce.toml");
        let output = openkeychron(&[
            "settings",
            "set",
            "--fixture",
            path.to_str().unwrap(),
            "debounce",
            "none",
            "0",
        ]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("--backup"), "{}", stderr(&output));
    }

    #[test]
    fn rgb_get_prints_the_lighting_values() {
        let path = fixture("cli-rgb-get.toml");
        let output = openkeychron(&["rgb", "get", "--fixture", path.to_str().unwrap()]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains("Brightness: 254"), "{text}");
        assert!(
            text.contains("Colour:     hue 170, saturation 255"),
            "{text}"
        );
    }

    #[test]
    fn rgb_set_brightness_saves_a_backup_and_prints_the_restore_value() {
        let path = fixture("cli-rgb-set-brightness.toml");
        let backup = std::env::temp_dir().join(format!(
            "openkeychron-rgb-backup-{}.json",
            std::process::id()
        ));
        let output = openkeychron(&[
            "rgb",
            "set",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "brightness",
            "128",
        ]);
        let saved = std::fs::read_to_string(&backup);
        std::fs::remove_file(&backup).ok();
        assert!(output.status.success(), "{}", stderr(&output));
        let saved: serde_json::Value = serde_json::from_str(&saved.unwrap()).unwrap();
        assert_eq!(saved["brightness"], 254);
        // 255 is the value that makes the keyboard store 254 again.
        assert!(
            stdout(&output).contains("brightness 255"),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn rgb_set_refuses_to_write_through_a_receiver() {
        let path = fixture("cli-info.toml");
        let backup = std::env::temp_dir().join(format!(
            "openkeychron-rgb-unused-{}.json",
            std::process::id()
        ));
        let output = openkeychron(&[
            "rgb",
            "set",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "speed",
            "10",
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("only written over a USB cable"),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }
}
