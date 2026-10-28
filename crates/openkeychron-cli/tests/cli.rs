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
}
