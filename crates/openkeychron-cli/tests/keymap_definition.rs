//! Drives `keymap show --layout` with a hand-written VIA definition and replay for a second
//! model, never a hardware session.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use openkeychron_hid::Fixture;
    use serde_json::Value;

    fn synthetic(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic")
            .join(name)
    }

    fn second_model_fixture() -> Fixture {
        Fixture::load(&synthetic("cli-keymap-second-model.toml")).unwrap()
    }

    fn second_model_definition() -> PathBuf {
        synthetic("via-definition-second-model.json")
    }

    fn temporary_path(extension: &str) -> PathBuf {
        static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);
        std::env::temp_dir().join(format!(
            "openkeychron-keymap-definition-{}-{}.{extension}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn run_fixture(arguments: &[&str], layout: &Path, fixture: &Fixture) -> Output {
        let path = temporary_path("toml");
        fixture.save(&path).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_openkeychron"))
            .args(arguments)
            .arg("--layout")
            .arg(layout)
            .arg("--fixture")
            .arg(&path)
            .output()
            .unwrap();
        std::fs::remove_file(path).unwrap();
        output
    }

    /// The second model's definition with `from` replaced by `to`, in a temporary file.
    fn edited_definition(from: &str, to: &str) -> PathBuf {
        let text = std::fs::read_to_string(second_model_definition()).unwrap();
        assert!(text.contains(from), "{from} is not in the definition");
        let path = temporary_path("json");
        std::fs::write(&path, text.replace(from, to)).unwrap();
        path
    }

    fn without_events(mut fixture: Fixture) -> Fixture {
        fixture.events.clear();
        fixture
    }

    fn stdout(output: &Output) -> String {
        String::from_utf8(output.stdout.clone()).unwrap()
    }

    fn stderr(output: &Output) -> String {
        String::from_utf8(output.stderr.clone()).unwrap()
    }

    fn assert_refused(output: &Output, expected: &str) {
        assert!(!output.status.success());
        assert_eq!(stdout(output), "");
        assert!(stderr(output).contains(expected), "{}", stderr(output));
    }

    #[test]
    fn json_uses_the_definition_matrix_size_and_reads_every_slot() {
        let output = run_fixture(
            &["keymap", "show", "--json"],
            &second_model_definition(),
            &second_model_fixture(),
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let json: Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["device"]["product_id"], 0x0950);
        assert_eq!(
            (json["rows"].clone(), json["columns"].clone()),
            (2.into(), 4.into())
        );
        let keycodes: Vec<_> = json["layers"][0]["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|key| key["keycode"].as_u64().unwrap())
            .collect();
        assert_eq!(keycodes, [0x29, 0x04, 0x05, 0xe2, 0x2b, 0x28, 0, 0]);
    }

    #[test]
    fn json_marks_which_positions_are_physical_keys_without_inventing_legends() {
        let output = run_fixture(
            &["keymap", "show", "--json"],
            &second_model_definition(),
            &second_model_fixture(),
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let json: Value = serde_json::from_str(&stdout(&output)).unwrap();
        let keys = json["layers"][0]["keys"].as_array().unwrap();
        let physical: Vec<_> = keys
            .iter()
            .map(|key| key["is_physical"].as_bool().unwrap())
            .collect();
        assert_eq!(physical, [true, true, true, true, true, true, false, true]);
        assert!(keys.iter().all(|key| key["legend"] == Value::Null));
    }

    #[test]
    fn text_grid_names_positions_and_shows_a_physical_key_assigned_zero() {
        let output = run_fixture(
            &["keymap", "show"],
            &second_model_definition(),
            &second_model_fixture(),
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        for expected in ["Layer 0", "(0,0)=0x0029", "(1,1)=0x0028", "(1,3)=0x0000"] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert!(!text.contains("(1,2)"), "{text}");
        assert!(!text.contains("FR legends"), "{text}");
    }

    #[test]
    fn a_model_without_a_known_layout_asks_for_its_definition_before_sending_reports() {
        let path = temporary_path("toml");
        without_events(second_model_fixture()).save(&path).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_openkeychron"))
            .args(["keymap", "show", "--fixture"])
            .arg(&path)
            .output()
            .unwrap();
        std::fs::remove_file(path).unwrap();
        assert_refused(&output, "no layout for 3434:0950; pass --layout");
    }

    #[test]
    fn a_definition_for_another_model_is_refused_before_sending_reports() {
        let layout = edited_definition(r#""0x0950""#, r#""0x0952""#);
        let output = run_fixture(
            &["keymap", "show"],
            &layout,
            &without_events(second_model_fixture()),
        );
        std::fs::remove_file(layout).unwrap();
        assert_refused(
            &output,
            "is for Synthetic second model (3434:0952), but the keyboard is 3434:0950",
        );
    }

    #[test]
    fn a_definition_does_not_replace_the_built_in_v5_max_iso_layout() {
        let layout = edited_definition(r#""0x0950""#, r#""0x0951""#);
        let mut fixture = without_events(second_model_fixture());
        fixture.device.product_id = 0x0951;
        let output = run_fixture(&["keymap", "show"], &layout, &fixture);
        std::fs::remove_file(layout).unwrap();
        assert_refused(&output, "already known");
    }

    #[test]
    fn a_receiver_is_refused_even_with_a_definition() {
        let mut fixture = without_events(second_model_fixture());
        fixture.device.product_id = 0xd030;
        let output = run_fixture(&["keymap", "show"], &second_model_definition(), &fixture);
        assert_refused(&output, "USB cable");
    }

    #[test]
    fn an_invalid_definition_names_the_file_and_the_problem() {
        let layout = edited_definition(r#""0,1""#, r#""0,1", {"r": 15}"#);
        let output = run_fixture(
            &["keymap", "show"],
            &layout,
            &without_events(second_model_fixture()),
        );
        std::fs::remove_file(&layout).unwrap();
        assert_refused(&output, &format!("layout file {}: ", layout.display()));
        assert!(
            stderr(&output).contains(r#"uses "r""#),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn a_missing_definition_file_is_named() {
        let layout = temporary_path("json");
        let output = run_fixture(
            &["keymap", "show"],
            &layout,
            &without_events(second_model_fixture()),
        );
        assert_refused(
            &output,
            &format!("cannot read layout file {}", layout.display()),
        );
    }
}
