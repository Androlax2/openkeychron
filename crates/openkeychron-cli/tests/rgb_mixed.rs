//! Runs `openkeychron rgb mixed` (mixed RGB regions and effect lists) against fixtures.

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

    /// The command failed after replaying the whole fixture, so it sent exactly what it records.
    fn assert_failed_at_the_end_of_the_fixture(output: &Output) {
        assert!(!output.status.success());
        assert!(
            !stderr(output).contains("also failed to finish the session"),
            "{}",
            stderr(output)
        );
    }

    fn new_backup_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "openkeychron-mixed-{name}-{}.json",
            std::process::id()
        ));
        std::fs::remove_file(&path).ok();
        path
    }

    fn read_json(path: &PathBuf) -> serde_json::Value {
        let text = std::fs::read_to_string(path).unwrap();
        std::fs::remove_file(path).ok();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn rgb_mixed_get_prints_each_regions_keys_and_effect_list() {
        let path = fixture("cli-rgb-mixed-get.toml");
        let output = openkeychron(&["rgb", "mixed", "get", "--fixture", path.to_str().unwrap()]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains("Region 1: Esc, F1, F2,"), "{text}");
        assert!(
            text.contains("  Slot 0: effect 2, hue 0, saturation 255, speed 127, 5000 ms"),
            "{text}"
        );
        assert!(text.contains("  Slot 4: empty"), "{text}");
    }

    #[test]
    fn rgb_mixed_get_prints_regions_as_json() {
        let path = fixture("cli-rgb-mixed-get.toml");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "get",
            "--json",
            "--fixture",
            path.to_str().unwrap(),
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["regions"][1]["keys"][0]["key"], "Esc");
        assert_eq!(json["regions"][1]["leds"].as_array().unwrap().len(), 16);
        assert_eq!(json["regions"][0]["effects"][0]["effect"], 5);
    }

    #[test]
    fn rgb_mixed_region_saves_the_whole_mixed_state_before_writing() {
        let path = fixture("cli-rgb-mixed-region.toml");
        let backup = new_backup_path("region");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "region",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "1",
            "A",
            "4,0",
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        let saved = read_json(&backup);
        assert_eq!(saved["regions"].as_array().unwrap().len(), 98);
        assert_eq!(saved["regions"][35], 0);
        assert_eq!(saved["effects"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn rgb_mixed_region_prints_the_restore_command() {
        let path = fixture("cli-rgb-mixed-region.toml");
        let backup = new_backup_path("region-restore");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "region",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "1",
            "A",
            "4,0",
        ]);
        std::fs::remove_file(&backup).ok();
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(
            stdout(&output).contains(&format!(
                "openkeychron rgb mixed restore --backup <NEW_FILE> {}",
                backup.display()
            )),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn rgb_mixed_region_sets_the_previous_state_again_after_a_read_back_mismatch() {
        let path = fixture("cli-rgb-mixed-region-mismatch.toml");
        let backup = new_backup_path("region-mismatch");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "region",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "1",
            "A",
            "4,0",
        ]);
        std::fs::remove_file(&backup).ok();
        assert_failed_at_the_end_of_the_fixture(&output);
        assert!(
            stderr(&output)
                .contains("the previous mixed RGB state is set again and nothing was saved"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn rgb_mixed_region_refuses_a_region_the_keyboard_lacks_before_reading_state() {
        let path = fixture("cli-rgb-mixed-open.toml");
        let backup = new_backup_path("region-missing");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "region",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "2",
            "A",
        ]);
        assert_failed_at_the_end_of_the_fixture(&output);
        assert!(
            stderr(&output).contains("mixed RGB region 2 does not exist; this keyboard has 2"),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }

    #[test]
    fn rgb_mixed_region_refuses_to_write_through_a_receiver() {
        let path = fixture("cli-info.toml");
        let backup = new_backup_path("region-receiver");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "region",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "1",
            "A",
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("mixed RGB writes require the V5 Max ISO (3434:0951)"),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }

    #[test]
    fn rgb_mixed_region_refuses_an_unknown_key_before_opening_the_device() {
        let backup = new_backup_path("region-unknown-key");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "region",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            "fixture-that-must-not-be-opened.toml",
            "1",
            "Menu",
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("no V5 Max ISO key has the legend `Menu`"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn rgb_mixed_region_refuses_an_existing_backup_and_keeps_its_content() {
        // The fixture holds only the reads before the backup, so any write would fail the replay.
        let path = fixture("cli-rgb-mixed-region-before-backup.toml");
        let backup = new_backup_path("region-existing");
        std::fs::write(&backup, "keep this existing backup").unwrap();
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "region",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "1",
            "A",
            "4,0",
        ]);
        let kept = std::fs::read_to_string(&backup);
        std::fs::remove_file(&backup).ok();
        assert_failed_at_the_end_of_the_fixture(&output);
        assert!(
            stderr(&output).contains(&format!(
                "cannot save new mixed RGB backup {}: ",
                backup.display()
            )),
            "{}",
            stderr(&output)
        );
        assert_eq!(kept.unwrap(), "keep this existing backup");
    }

    #[test]
    fn rgb_mixed_effect_sets_one_slot_and_saves() {
        let path = fixture("cli-rgb-mixed-effect.toml");
        let backup = new_backup_path("effect");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "effect",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "1",
            "1",
            "4",
            "85",
            "200",
            "64",
            "3000",
        ]);
        std::fs::remove_file(&backup).ok();
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(
            stdout(&output).contains(
                "Region 1, slot 1 set to effect 4, hue 85, saturation 200, speed 64, 3000 ms"
            ),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn rgb_mixed_effect_refuses_a_slot_the_keyboard_lacks_before_reading_state() {
        let path = fixture("cli-rgb-mixed-open.toml");
        let backup = new_backup_path("effect-missing");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "effect",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            "0",
            "5",
            "4",
            "0",
            "0",
            "0",
            "1000",
        ]);
        assert_failed_at_the_end_of_the_fixture(&output);
        assert!(
            stderr(&output).contains("effect slot 5 does not exist; each region has 5"),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }

    #[test]
    fn rgb_mixed_restore_writes_a_backup_after_saving_the_current_state() {
        let path = fixture("cli-rgb-mixed-restore.toml");
        let source = fixture("mixed-rgb-backup.json");
        let backup = new_backup_path("pre-restore");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "restore",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            source.to_str().unwrap(),
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        let saved = read_json(&backup);
        assert_eq!(saved["regions"][0], 1);
    }

    #[test]
    fn rgb_mixed_restore_refuses_a_backup_with_another_led_count() {
        let path = fixture("cli-rgb-mixed-open.toml");
        let mut source: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(fixture("mixed-rgb-backup.json")).unwrap(),
        )
        .unwrap();
        source["led_count"] = 97.into();
        source["regions"].as_array_mut().unwrap().pop();
        let source_path = new_backup_path("restore-source-97");
        std::fs::write(&source_path, source.to_string()).unwrap();
        let backup = new_backup_path("restore-97");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "restore",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            path.to_str().unwrap(),
            source_path.to_str().unwrap(),
        ]);
        std::fs::remove_file(&source_path).ok();
        assert_failed_at_the_end_of_the_fixture(&output);
        assert!(
            stderr(&output).contains("holds 97 LEDs; this keyboard has 98"),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }

    #[test]
    fn rgb_mixed_restore_refuses_a_backup_whose_regions_do_not_match_its_led_count() {
        let mut source: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(fixture("mixed-rgb-backup.json")).unwrap(),
        )
        .unwrap();
        source["regions"].as_array_mut().unwrap().pop();
        let source_path = new_backup_path("restore-source-short");
        std::fs::write(&source_path, source.to_string()).unwrap();
        let backup = new_backup_path("restore-short");
        let output = openkeychron(&[
            "rgb",
            "mixed",
            "restore",
            "--backup",
            backup.to_str().unwrap(),
            "--fixture",
            "fixture-that-must-not-be-opened.toml",
            source_path.to_str().unwrap(),
        ]);
        std::fs::remove_file(&source_path).ok();
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("has 97 regions for 98 LEDs"),
            "{}",
            stderr(&output)
        );
        assert!(!backup.exists());
    }
}
