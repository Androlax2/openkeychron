//! `macro list`, `macro set` and `macro restore` through the real binary, replaying
//! hand-written synthetic sessions only.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use openkeychron_hid::{Event, Fixture};
    use serde_json::{Value, json};

    /// Events in one capture: count, size and two buffer chunks, each a write and a read.
    const CAPTURE_EVENTS: usize = 8;

    struct Scratch {
        directory: PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
            let directory = std::env::temp_dir().join(format!(
                "openkeychron-macros-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&directory).unwrap();
            Self { directory }
        }

        fn path(&self, name: &str) -> PathBuf {
            self.directory.join(name)
        }

        fn source(&self, value: &Value) -> PathBuf {
            let path = self.path("source.json");
            std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.directory).unwrap();
        }
    }

    fn fixture(name: &str) -> Fixture {
        Fixture::load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/synthetic")
                .join(name),
        )
        .unwrap()
    }

    fn capture_only(name: &str) -> Fixture {
        let mut fixture = fixture(name);
        fixture.events.truncate(CAPTURE_EVENTS);
        fixture
    }

    fn no_reports(product_id: u16) -> Fixture {
        let mut fixture = fixture("cli-macro-set.toml");
        fixture.events.clear();
        fixture.device.product_id = product_id;
        fixture
    }

    /// Sets the last byte of the buffer in the first capture, which the firmware treats as
    /// an unfinished write.
    fn with_macros_disabled(mut fixture: Fixture) -> Fixture {
        reply(&mut fixture, 7)[4 + 11] = 0xff;
        fixture
    }

    fn original_backup() -> Value {
        serde_json::from_str(include_str!(
            "../../../fixtures/synthetic/macro-backup.json"
        ))
        .unwrap()
    }

    fn saved_backup(path: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn run(scratch: &Scratch, fixture: &Fixture, arguments: &[&str]) -> Output {
        let path = scratch.path("session.toml");
        fixture.save(&path).unwrap();
        Command::new(env!("CARGO_BIN_EXE_openkeychron"))
            .args(arguments)
            .arg("--fixture")
            .arg(&path)
            .output()
            .unwrap()
    }

    fn set(scratch: &Scratch, fixture: &Fixture, backup: &Path) -> Output {
        run(
            scratch,
            fixture,
            &[
                "macro",
                "set",
                "1",
                "text:hi",
                "tap:0x2c",
                "delay:50",
                "--backup",
                backup.to_str().unwrap(),
            ],
        )
    }

    fn stdout(output: &Output) -> String {
        String::from_utf8(output.stdout.clone()).unwrap()
    }

    fn stderr(output: &Output) -> String {
        String::from_utf8(output.stderr.clone()).unwrap()
    }

    fn failed(output: &Output, expected: &str) {
        assert!(!output.status.success());
        assert_eq!(stdout(output), "");
        assert!(stderr(output).contains(expected), "{}", stderr(output));
    }

    fn reply(fixture: &mut Fixture, index: usize) -> &mut [u8] {
        let Event::Read { data } = &mut fixture.events[index] else {
            panic!("expected synthetic reply at event {index}");
        };
        data
    }

    #[test]
    fn macro_list_shows_each_slot_as_actions() {
        let scratch = Scratch::new();
        let output = run(
            &scratch,
            &fixture("cli-macro-list.toml"),
            &["macro", "list"],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains("M0  text:\"hello\" tap:0x28\n"), "{text}");
        assert!(text.contains("M1  (empty)\n"), "{text}");
        assert!(
            text.contains("M2  down:0xe1 text:\"a\" up:0xe1 delay:100\n"),
            "{text}"
        );
    }

    #[test]
    fn macro_list_shows_an_unreadable_slot_with_its_raw_bytes() {
        let scratch = Scratch::new();
        let output = run(
            &scratch,
            &fixture("cli-macro-list.toml"),
            &["macro", "list"],
        );
        let text = stdout(&output);
        assert!(text.contains("M3  unreadable: "), "{text}");
        assert!(text.contains("unknown code 5"), "{text}");
        assert!(text.contains("raw bytes: 01 05 78\n"), "{text}");
    }

    #[test]
    fn macro_list_json_gives_actions_errors_and_raw_bytes() {
        let scratch = Scratch::new();
        let output = run(
            &scratch,
            &fixture("cli-macro-list.toml"),
            &["macro", "list", "--json"],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let json: Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["macro_count"], 4);
        assert_eq!(json["buffer_size"], 40);
        assert_eq!(json["macros_enabled"], true);
        assert_eq!(
            json["macros"][0],
            json!({"slot": 0, "actions": [{"text": "hello"}, {"tap": 40}], "error": null,
                   "bytes": [104, 101, 108, 108, 111, 1, 1, 40]})
        );
        assert_eq!(
            json["macros"][2]["actions"],
            json!([{"down": 225}, {"text": "a"}, {"up": 225}, {"delay": 100}])
        );
        assert_eq!(json["macros"][3]["actions"], Value::Null);
        assert_eq!(json["macros"][3]["bytes"], json!([1, 5, 120]));
    }

    #[test]
    fn macro_list_warns_that_a_non_zero_last_byte_disables_every_macro() {
        let scratch = Scratch::new();
        let output = run(
            &scratch,
            &with_macros_disabled(fixture("cli-macro-list.toml")),
            &["macro", "list"],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(
            stdout(&output).contains("the firmware plays no macro"),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn macro_set_saves_the_whole_buffer_before_writing_one_slot() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let output = set(&scratch, &fixture("cli-macro-set.toml"), &backup);
        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(saved_backup(&backup), original_backup());
        assert!(
            stdout(&output).contains(&format!("Pre-change backup: {}", backup.display())),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn macro_set_json_names_the_operation_and_backup() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let output = run(
            &scratch,
            &fixture("cli-macro-set.toml"),
            &[
                "macro",
                "set",
                "1",
                "text:hi",
                "tap:0x2c",
                "delay:50",
                "--backup",
                backup.to_str().unwrap(),
                "--json",
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let json: Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json, json!({"operation": "set", "backup": backup}));
    }

    #[test]
    fn macro_set_refuses_an_existing_backup_path_before_any_write() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        std::fs::write(&backup, "keep me").unwrap();
        let output = set(&scratch, &capture_only("cli-macro-set.toml"), &backup);
        failed(&output, "no write was sent to the keyboard");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "keep me");
    }

    #[test]
    fn macro_set_refuses_the_receiver_and_other_models_before_any_report() {
        for product_id in [0xd030, 0x0950] {
            let scratch = Scratch::new();
            let backup = scratch.path("before.json");
            let output = set(&scratch, &no_reports(product_id), &backup);
            failed(&output, "3434:0951");
            assert!(!backup.exists());
        }
    }

    #[test]
    fn macro_set_refuses_a_slot_beyond_the_macro_count_without_a_backup() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let output = run(
            &scratch,
            &capture_only("cli-macro-set.toml"),
            &[
                "macro",
                "set",
                "4",
                "tap:4",
                "--backup",
                backup.to_str().unwrap(),
            ],
        );
        failed(&output, "slot 4 is unavailable; select 0..=3");
        assert!(!backup.exists());
    }

    #[test]
    fn macro_set_refuses_macros_that_do_not_fit_the_buffer_without_a_backup() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let output = run(
            &scratch,
            &capture_only("cli-macro-set.toml"),
            &[
                "macro",
                "set",
                "1",
                "text:this text is far too long",
                "--backup",
                backup.to_str().unwrap(),
            ],
        );
        failed(&output, "macro buffer holds 40");
        assert!(!backup.exists());
    }

    #[test]
    fn macro_set_refuses_while_the_last_byte_disables_every_macro() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let output = set(
            &scratch,
            &with_macros_disabled(capture_only("cli-macro-set.toml")),
            &backup,
        );
        failed(&output, "macro restore");
        assert!(!backup.exists());
    }

    #[test]
    fn macro_set_refuses_invalid_actions_before_opening_a_session() {
        for action in ["press:4", "tap:0", "tap:256", "delay:-1", "text:é", "hello"] {
            let scratch = Scratch::new();
            let backup = scratch.path("before.json");
            let output = run(
                &scratch,
                &no_reports(0x0951),
                &[
                    "macro",
                    "set",
                    "0",
                    action,
                    "--backup",
                    backup.to_str().unwrap(),
                ],
            );
            assert!(!output.status.success(), "{action}");
            assert!(!backup.exists(), "{action}");
        }
    }

    #[test]
    fn macro_set_read_back_mismatch_keeps_the_backup_and_reports_possible_change() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let mut fixture = fixture("cli-macro-set.toml");
        fixture.events.truncate(18);
        reply(&mut fixture, 17)[4 + 9] = b'X';
        let output = set(&scratch, &fixture, &backup);
        failed(&output, "may have changed");
        assert!(stderr(&output).contains(&format!(
            "openkeychron macro restore {} --backup <NEW_FILE>",
            backup.display()
        )));
        assert_eq!(saved_backup(&backup), original_backup());
    }

    #[test]
    fn macro_set_detects_a_buffer_that_differs_after_writing() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let mut fixture = fixture("cli-macro-set.toml");
        let final_chunk = fixture.events.len() - 3;
        reply(&mut fixture, final_chunk)[4] = b'j';
        let output = set(&scratch, &fixture, &backup);
        failed(&output, "verification");
        assert!(stderr(&output).contains("may have changed"));
    }

    #[test]
    fn macro_restore_saves_a_separate_backup_then_writes_the_source() {
        let scratch = Scratch::new();
        let source = scratch.source(&original_backup());
        let before = scratch.path("before-restore.json");
        let output = run(
            &scratch,
            &fixture("cli-macro-restore.toml"),
            &[
                "macro",
                "restore",
                source.to_str().unwrap(),
                "--backup",
                before.to_str().unwrap(),
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let previous = saved_backup(&before);
        assert_eq!(previous["buffer"][9], u32::from(b'h'));
        assert_eq!(saved_backup(&source), original_backup());
    }

    #[test]
    fn macro_restore_refuses_a_backup_for_another_buffer_size_without_a_backup() {
        let scratch = Scratch::new();
        let mut wanted = original_backup();
        wanted["buffer_size"] = json!(41);
        wanted["buffer"].as_array_mut().unwrap().push(json!(0));
        let source = scratch.source(&wanted);
        let before = scratch.path("before-restore.json");
        let output = run(
            &scratch,
            &capture_only("cli-macro-restore.toml"),
            &[
                "macro",
                "restore",
                source.to_str().unwrap(),
                "--backup",
                before.to_str().unwrap(),
            ],
        );
        failed(&output, "do not match");
        assert!(!before.exists());
    }

    #[test]
    fn macro_restore_refuses_invalid_backups_before_opening_a_session() {
        let original = original_backup();
        let mut invalid = Vec::new();
        for field in [
            "format_version",
            "device",
            "macro_count",
            "buffer_size",
            "buffer",
        ] {
            let mut value = original.clone();
            value.as_object_mut().unwrap().remove(field);
            invalid.push(value);
        }
        for (pointer, replacement) in [
            ("/format_version", json!(2)),
            ("/device/product_id", json!(0xd030)),
            ("/buffer_size", json!(39)),
            ("/buffer_size", json!(0)),
            ("/buffer/0", json!(256)),
            ("/buffer/39", json!(255)),
        ] {
            let mut value = original.clone();
            *value.pointer_mut(pointer).unwrap() = replacement;
            invalid.push(value);
        }
        let mut value = original.clone();
        value["extra"] = json!(true);
        invalid.push(value);
        for value in invalid {
            let scratch = Scratch::new();
            let source = scratch.source(&value);
            let before = scratch.path("before-restore.json");
            let output = run(
                &scratch,
                &no_reports(0x0951),
                &[
                    "macro",
                    "restore",
                    source.to_str().unwrap(),
                    "--backup",
                    before.to_str().unwrap(),
                ],
            );
            failed(&output, "macro backup");
            assert!(!before.exists(), "{value}");
        }
    }

    #[test]
    fn macro_writes_refuse_recording_before_opening_a_session() {
        for arguments in [
            vec!["macro", "set", "0", "tap:4", "--backup", "before.json"],
            vec!["macro", "restore", "source.json", "--backup", "before.json"],
        ] {
            let scratch = Scratch::new();
            let recording = scratch.path("recording.toml");
            let output = Command::new(env!("CARGO_BIN_EXE_openkeychron"))
                .current_dir(&scratch.directory)
                .args(&arguments)
                .arg("--record")
                .arg(&recording)
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(stderr(&output).contains("--record"), "{}", stderr(&output));
            assert!(!recording.exists());
        }
    }
}
