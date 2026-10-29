//! Drives the actual CLI with synthetic keymap traffic, never a hardware session.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use openkeychron_hid::{Event, Fixture};

    fn fixture() -> Fixture {
        Fixture::load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/synthetic/cli-keymap.toml"),
        )
        .unwrap()
    }

    fn run_fixture(arguments: &[&str], fixture: &Fixture) -> Output {
        static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "openkeychron-keymap-{}-{}.toml",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        fixture.save(&path).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_openkeychron"))
            .args(arguments)
            .arg("--fixture")
            .arg(&path)
            .output()
            .unwrap();
        std::fs::remove_file(path).unwrap();
        output
    }

    fn stdout(output: &Output) -> String {
        String::from_utf8(output.stdout.clone()).unwrap()
    }

    fn stderr(output: &Output) -> String {
        String::from_utf8(output.stderr.clone()).unwrap()
    }

    #[test]
    fn keymap_show_prints_all_layers_with_french_positions_and_raw_assignments() {
        let output = run_fixture(&["keymap", "show"], &fixture());
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        for expected in [
            "Layer 0",
            "Layer 1",
            "A=0x0014",
            "Q=0x0004",
            "M=0x0033",
            "W=0x001d",
            "R=0x0000",
            "(0,1)=0xdead",
            "(5,18)=0xbeef",
            "M=0x7e00",
            "(5,18)=0xffff",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert_eq!(
            text.lines().filter(|line| line.starts_with('r')).count(),
            12
        );
    }

    #[test]
    fn keymap_show_json_preserves_matrix_positions_gaps_and_every_raw_value() {
        let output = run_fixture(&["keymap", "show", "--json"], &fixture());
        assert!(output.status.success(), "{}", stderr(&output));
        let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["device"]["product_id"], 0x0951);
        assert_eq!(json["layer_count"], 2);
        assert_eq!(json["rows"], 6);
        assert_eq!(json["columns"], 19);
        assert_eq!(json["layers"].as_array().unwrap().len(), 2);
        for layer in json["layers"].as_array().unwrap() {
            assert_eq!(layer["keys"].as_array().unwrap().len(), 114);
        }
        let keys = &json["layers"][0]["keys"];
        assert_eq!(keys[39]["row"], 2);
        assert_eq!(keys[39]["column"], 1);
        assert_eq!(keys[39]["legend"], "A");
        assert_eq!(keys[39]["keycode"], 0x0014);
        assert_eq!(keys[67]["legend"], "M");
        assert_eq!(keys[67]["keycode"], 0x0033);
        assert_eq!(keys[78]["legend"], "W");
        assert_eq!(keys[78]["keycode"], 0x001d);
        assert_eq!(keys[1]["legend"], serde_json::Value::Null);
        assert_eq!(keys[1]["keycode"], 0xdead);
        assert_eq!(keys[6]["legend"], serde_json::Value::Null);
        assert_eq!(keys[6]["keycode"], 0);
        assert_eq!(keys[113]["keycode"], 0xbeef);
        let keys = &json["layers"][1]["keys"];
        assert_eq!(keys[0]["keycode"], 1);
        assert_eq!(keys[39]["keycode"], 4);
        assert_eq!(keys[67]["keycode"], 0x7e00);
        assert_eq!(keys[113]["keycode"], 0xffff);
    }

    #[test]
    fn keymap_show_layer_filters_json_without_changing_the_replay_sequence() {
        let output = run_fixture(&["keymap", "show", "--layer", "1", "--json"], &fixture());
        assert!(output.status.success(), "{}", stderr(&output));
        let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["layer_count"], 2);
        assert_eq!(json["layers"].as_array().unwrap().len(), 1);
        assert_eq!(json["layers"][0]["index"], 1);
        assert_eq!(json["layers"][0]["keys"][39]["keycode"], 4);
    }

    #[test]
    fn keymap_show_layer_filters_the_text_grid() {
        let output = run_fixture(&["keymap", "show", "--layer", "1"], &fixture());
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains("Layer 1"), "{text}");
        assert!(!text.contains("Layer 0"), "{text}");
        assert!(text.contains("Esc=0x0001"), "{text}");
        assert!(text.contains("M=0x7e00"), "{text}");
    }

    #[test]
    fn keymap_show_rejects_an_unavailable_layer_before_reading_the_buffer() {
        let mut fixture = fixture();
        fixture.events.truncate(2);
        let output = run_fixture(&["keymap", "show", "--layer", "2"], &fixture);
        assert!(!output.status.success());
        assert_eq!(stdout(&output), "");
        let error = stderr(&output);
        assert!(
            error.contains("layer 2") && error.contains("0..=1"),
            "{error}"
        );
    }

    #[test]
    fn keymap_show_rejects_invalid_layer_arguments_without_opening_hardware() {
        for layer in ["-1", "256", "not-a-layer"] {
            let output = Command::new(env!("CARGO_BIN_EXE_openkeychron"))
                .args(["keymap", "show", &format!("--layer={layer}")])
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(stderr(&output).contains("--layer"), "{}", stderr(&output));
        }
    }

    #[test]
    fn keymap_show_refuses_unknown_layouts_and_receivers_before_sending_reports() {
        for (vendor_id, product_id, expected) in [
            (0x3434, 0x0950, "V5 Max ISO"),
            (0x3434, 0xd030, "V5 Max ISO"),
            (0x1234, 0x0951, "Keychron VIA interface"),
        ] {
            let mut fixture = fixture();
            fixture.device.vendor_id = vendor_id;
            fixture.device.product_id = product_id;
            fixture.events.clear();
            let output = run_fixture(&["keymap", "show"], &fixture);
            assert!(!output.status.success());
            assert_eq!(stdout(&output), "");
            assert!(stderr(&output).contains(expected), "{}", stderr(&output));
        }
    }

    #[test]
    fn keymap_show_reports_an_incomplete_replay_without_printing_success() {
        let mut fixture = fixture();
        fixture.events.push(Event::ReadTimeout);
        let output = run_fixture(&["keymap", "show", "--json"], &fixture);
        assert!(!output.status.success());
        assert_eq!(stdout(&output), "");
        assert!(
            stderr(&output).contains("never reached"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn keymap_show_reports_a_later_read_timeout_without_partial_output() {
        let mut fixture = fixture();
        fixture.events[5] = Event::ReadTimeout;
        let output = run_fixture(&["keymap", "show"], &fixture);
        assert!(!output.status.success());
        assert_eq!(stdout(&output), "");
        assert!(
            stderr(&output).contains("no input report arrived before the timeout"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn keymap_show_rejects_a_zero_layer_count_without_reading_the_buffer() {
        let mut fixture = fixture();
        fixture.events.truncate(2);
        let Event::Read { data } = &mut fixture.events[1] else {
            panic!("expected a synthetic layer-count reply");
        };
        data[1] = 0;
        let output = run_fixture(&["keymap", "show"], &fixture);
        assert!(!output.status.success());
        assert_eq!(stdout(&output), "");
        assert!(
            stderr(&output).contains("at least one"),
            "{}",
            stderr(&output)
        );
    }
}
