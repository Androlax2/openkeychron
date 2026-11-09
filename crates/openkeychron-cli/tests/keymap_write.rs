//! Exercises mutating CLI paths only with explicit synthetic replay transports.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use openkeychron_hid::{Event, Fixture};
    use serde_json::{Value, json};

    struct Scratch {
        directory: PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
            let directory = std::env::temp_dir().join(format!(
                "openkeychron-keymap-write-{}-{}",
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

    fn original_backup() -> Value {
        serde_json::from_str(include_str!(
            "../../../fixtures/synthetic/keymap-backup.json"
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

    fn capture_only(name: &str) -> Fixture {
        let mut fixture = fixture(name);
        fixture.events.truncate(26);
        fixture
    }

    #[test]
    fn write_set_saves_all_matrix_and_encoder_state_before_changing_one_key() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let output = run(
            &scratch,
            &fixture("cli-keymap-set.toml"),
            &[
                "keymap",
                "set",
                "0",
                "2,1",
                "0x0004",
                "--backup",
                backup.to_str().unwrap(),
                "--json",
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(saved_backup(&backup), original_backup());
        let json: Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["operation"], "set");
    }

    #[test]
    fn write_restore_preserves_a_separate_pre_change_backup_and_restores_encoders() {
        let scratch = Scratch::new();
        let source = scratch.source(&original_backup());
        let before = scratch.path("before-restore.json");
        let output = run(
            &scratch,
            &fixture("cli-keymap-restore.toml"),
            &[
                "keymap",
                "restore",
                source.to_str().unwrap(),
                "--backup",
                before.to_str().unwrap(),
                "--json",
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let previous = saved_backup(&before);
        assert_eq!(previous["matrix"][1], 0xffff);
        assert_eq!(previous["matrix"][39], 4);
        assert_eq!(previous["matrix"][113], 0);
        assert_eq!(previous["encoders"][0]["clockwise"], 0x4444);
        assert_eq!(previous["encoders"][0]["counter_clockwise"], 0x5555);
        assert_eq!(saved_backup(&source), original_backup());
        let json: Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["operation"], "restore");
    }

    #[test]
    fn write_restore_refuses_incomplete_malformed_or_mismatched_backup_data_before_reports() {
        let original = original_backup();
        let mut invalid = Vec::new();
        for field in [
            "format_version",
            "device",
            "layout",
            "layout_options",
            "rows",
            "columns",
            "layer_count",
            "encoder_count",
            "matrix",
            "encoders",
        ] {
            let mut value = original.clone();
            value.as_object_mut().unwrap().remove(field);
            invalid.push(value);
        }
        for (pointer, replacement) in [
            ("/format_version", json!(2)),
            ("/layout", json!("other-layout")),
            ("/layout_options", json!(1)),
            ("/rows", json!(5)),
            ("/columns", json!(18)),
            ("/layer_count", json!(0)),
            ("/encoder_count", json!(2)),
            ("/device/vendor_id", json!(0x1234)),
            ("/device/product_id", json!(0xd030)),
            ("/matrix/0", json!(-1)),
            ("/matrix/0", json!(65536)),
            ("/encoders/0/clockwise", json!(65536)),
        ] {
            let mut value = original.clone();
            *value.pointer_mut(pointer).unwrap() = replacement;
            invalid.push(value);
        }
        let mut value = original.clone();
        value["matrix"].as_array_mut().unwrap().pop();
        invalid.push(value);
        let mut value = original.clone();
        value["encoders"] = json!([]);
        invalid.push(value);
        let mut value = original.clone();
        value["encoders"][0]
            .as_object_mut()
            .unwrap()
            .remove("counter_clockwise");
        invalid.push(value);
        let mut value = original.clone();
        value["unexpected"] = json!(true);
        invalid.push(value);
        let mut value = original.clone();
        value["device"]["unexpected"] = json!(true);
        invalid.push(value);
        let mut value = original;
        value["encoders"][0]["unexpected"] = json!(true);
        invalid.push(value);

        let mut no_reports = fixture("cli-keymap-restore.toml");
        no_reports.events.clear();
        for value in invalid {
            let scratch = Scratch::new();
            let source = scratch.source(&value);
            let before = scratch.path("before.json");
            let output = run(
                &scratch,
                &no_reports,
                &[
                    "keymap",
                    "restore",
                    source.to_str().unwrap(),
                    "--backup",
                    before.to_str().unwrap(),
                ],
            );
            failed(&output, "invalid keymap backup");
            assert!(!before.exists());
        }
    }

    #[test]
    fn write_restore_refuses_unreadable_invalid_json_and_oversized_sources() {
        let mut no_reports = fixture("cli-keymap-restore.toml");
        no_reports.events.clear();
        for contents in [
            None,
            Some(b"{invalid JSON}".to_vec()),
            Some(vec![b' '; 1_048_577]),
        ] {
            let scratch = Scratch::new();
            let source = scratch.path("source.json");
            if let Some(contents) = contents {
                std::fs::write(&source, contents).unwrap();
            }
            let before = scratch.path("before.json");
            let output = run(
                &scratch,
                &no_reports,
                &[
                    "keymap",
                    "restore",
                    source.to_str().unwrap(),
                    "--backup",
                    before.to_str().unwrap(),
                ],
            );
            failed(&output, "keymap backup");
            assert!(!before.exists());
        }
    }

    #[test]
    fn write_set_refuses_existing_or_uncreatable_backup_paths_before_any_write() {
        for exists in [false, true] {
            let scratch = Scratch::new();
            let backup = if exists {
                scratch.path("existing.json")
            } else {
                scratch.path("missing/before.json")
            };
            if exists {
                std::fs::write(&backup, "keep this existing backup").unwrap();
            }
            let output = run(
                &scratch,
                &capture_only("cli-keymap-set.toml"),
                &[
                    "keymap",
                    "set",
                    "0",
                    "2,1",
                    "4",
                    "--backup",
                    backup.to_str().unwrap(),
                ],
            );
            failed(&output, "cannot save new keymap backup");
            assert!(
                !stderr(&output).contains("unexpected write"),
                "{}",
                stderr(&output)
            );
            if exists {
                assert_eq!(
                    std::fs::read_to_string(&backup).unwrap(),
                    "keep this existing backup"
                );
            } else {
                assert!(!backup.exists());
            }
        }
    }

    #[test]
    fn write_restore_never_overwrites_its_source_backup() {
        let scratch = Scratch::new();
        let source = scratch.source(&original_backup());
        let output = run(
            &scratch,
            &capture_only("cli-keymap-restore.toml"),
            &[
                "keymap",
                "restore",
                source.to_str().unwrap(),
                "--backup",
                source.to_str().unwrap(),
            ],
        );
        failed(&output, "cannot save new keymap backup");
        assert_eq!(saved_backup(&source), original_backup());
    }

    #[test]
    fn write_set_requires_encoder_backup_support_before_any_key_write() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let mut fixture = fixture("cli-keymap-set.toml");
        fixture.events.truncate(24);
        reply(&mut fixture, 23)[0] = 0xff;
        let output = run(
            &scratch,
            &fixture,
            &[
                "keymap",
                "set",
                "0",
                "2,1",
                "4",
                "--backup",
                backup.to_str().unwrap(),
            ],
        );
        failed(&output, "0x14");
        assert!(!backup.exists());
    }

    #[test]
    fn write_commands_keep_receiver_and_unknown_layout_refusals_before_reports() {
        for product_id in [0xd030, 0x0950] {
            for operation in ["set", "restore"] {
                let scratch = Scratch::new();
                let backup = scratch.path("before.json");
                let source = scratch.source(&original_backup());
                let mut fixture = fixture("cli-keymap-set.toml");
                fixture.device.product_id = product_id;
                fixture.events.clear();
                let output = if operation == "set" {
                    run(
                        &scratch,
                        &fixture,
                        &[
                            "keymap",
                            "set",
                            "0",
                            "2,1",
                            "4",
                            "--backup",
                            backup.to_str().unwrap(),
                        ],
                    )
                } else {
                    run(
                        &scratch,
                        &fixture,
                        &[
                            "keymap",
                            "restore",
                            source.to_str().unwrap(),
                            "--backup",
                            backup.to_str().unwrap(),
                        ],
                    )
                };
                failed(&output, "USB cable");
                assert!(!backup.exists());
            }
        }
    }

    #[test]
    fn write_restore_refuses_target_layer_count_or_layout_option_mismatch() {
        for mismatch in ["layers", "options"] {
            let scratch = Scratch::new();
            let source = scratch.source(&original_backup());
            let before = scratch.path("before.json");
            let mut fixture = fixture("cli-keymap-restore.toml");
            fixture.events.truncate(4);
            if mismatch == "layers" {
                reply(&mut fixture, 1)[1] = 2;
            } else {
                reply(&mut fixture, 3)[5] = 1;
            }
            let output = run(
                &scratch,
                &fixture,
                &[
                    "keymap",
                    "restore",
                    source.to_str().unwrap(),
                    "--backup",
                    before.to_str().unwrap(),
                ],
            );
            failed(&output, "match");
            assert!(!before.exists());
        }
    }

    #[test]
    fn write_set_read_back_mismatch_retains_backup_and_reports_uncertain_state() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let mut fixture = fixture("cli-keymap-set.toml");
        fixture.events.truncate(30);
        reply(&mut fixture, 29)[5] = 5;
        let output = run(
            &scratch,
            &fixture,
            &[
                "keymap",
                "set",
                "0",
                "2,1",
                "4",
                "--backup",
                backup.to_str().unwrap(),
                "--json",
            ],
        );
        failed(&output, "may have changed");
        assert!(stderr(&output).contains("0x0005"));
        assert_eq!(saved_backup(&backup), original_backup());
    }

    #[test]
    fn write_restore_partial_matrix_failure_retains_pre_restore_state_without_success() {
        let scratch = Scratch::new();
        let source = scratch.source(&original_backup());
        let before = scratch.path("before.json");
        let mut fixture = fixture("cli-keymap-restore.toml");
        fixture.events.truncate(34);
        reply(&mut fixture, 33)[4] = 0xff;
        let output = run(
            &scratch,
            &fixture,
            &[
                "keymap",
                "restore",
                source.to_str().unwrap(),
                "--backup",
                before.to_str().unwrap(),
            ],
        );
        failed(&output, "may have changed");
        assert_eq!(saved_backup(&before)["matrix"][1], 0xffff);
        assert_eq!(saved_backup(&before)["encoders"][0]["clockwise"], 0x4444);
        assert_eq!(saved_backup(&source), original_backup());
    }

    #[test]
    fn write_restore_partial_encoder_failure_never_claims_full_restoration() {
        let scratch = Scratch::new();
        let source = scratch.source(&original_backup());
        let before = scratch.path("before.json");
        let mut fixture = fixture("cli-keymap-restore.toml");
        fixture.events.truncate(70);
        reply(&mut fixture, 69)[4..6].copy_from_slice(&[0, 0]);
        let output = run(
            &scratch,
            &fixture,
            &[
                "keymap",
                "restore",
                source.to_str().unwrap(),
                "--backup",
                before.to_str().unwrap(),
            ],
        );
        failed(&output, "may have changed");
        assert!(stderr(&output).contains("0xabcd"));
        assert_eq!(
            saved_backup(&before)["encoders"][0]["counter_clockwise"],
            0x5555
        );
    }

    #[test]
    fn write_set_detects_changes_outside_the_selected_key_in_final_verification() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let mut fixture = fixture("cli-keymap-set.toml");
        reply(&mut fixture, 35)[5] = 0x2a;
        let output = run(
            &scratch,
            &fixture,
            &[
                "keymap",
                "set",
                "0",
                "2,1",
                "4",
                "--backup",
                backup.to_str().unwrap(),
            ],
        );
        failed(&output, "verification");
        assert!(stderr(&output).contains("may have changed"));
        assert_eq!(saved_backup(&backup), original_backup());
    }

    #[test]
    fn write_success_output_waits_for_replay_finalization() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let mut fixture = fixture("cli-keymap-set.toml");
        fixture.events.push(Event::ReadTimeout);
        let output = run(
            &scratch,
            &fixture,
            &[
                "keymap",
                "set",
                "0",
                "2,1",
                "4",
                "--backup",
                backup.to_str().unwrap(),
                "--json",
            ],
        );
        failed(&output, "never reached");
        assert!(stderr(&output).contains("may have changed"));
        assert_eq!(saved_backup(&backup), original_backup());
    }

    fn reset(scratch: &Scratch, fixture: &Fixture, backup: &Path) -> Output {
        run(
            scratch,
            fixture,
            &[
                "keymap",
                "reset",
                "--backup",
                backup.to_str().unwrap(),
                "--json",
            ],
        )
    }

    #[test]
    fn write_reset_saves_the_complete_keymap_before_sending_the_reset() {
        let scratch = Scratch::new();
        let backup = scratch.path("before-reset.json");
        let output = reset(&scratch, &fixture("cli-keymap-reset.toml"), &backup);
        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(saved_backup(&backup), original_backup());
        let json: Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["operation"], "reset");
        assert_eq!(json["backup"], backup.to_str().unwrap());
    }

    #[test]
    fn write_reset_refuses_an_existing_backup_path_before_sending_the_reset() {
        let scratch = Scratch::new();
        let backup = scratch.path("existing.json");
        std::fs::write(&backup, "keep this existing backup").unwrap();
        let output = reset(&scratch, &capture_only("cli-keymap-reset.toml"), &backup);
        failed(&output, "cannot save new keymap backup");
        assert!(
            !stderr(&output).contains("unexpected write"),
            "{}",
            stderr(&output)
        );
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            "keep this existing backup"
        );
    }

    #[test]
    fn write_reset_refuses_the_receiver_before_reports() {
        let scratch = Scratch::new();
        let backup = scratch.path("before-reset.json");
        let mut fixture = fixture("cli-keymap-reset.toml");
        fixture.device.product_id = 0xd030;
        fixture.events.clear();
        let output = reset(&scratch, &fixture, &backup);
        failed(&output, "USB cable");
        assert!(!backup.exists());
    }

    #[test]
    fn write_reset_rejected_by_the_firmware_retains_the_backup_without_success() {
        let scratch = Scratch::new();
        let backup = scratch.path("before-reset.json");
        let mut fixture = fixture("cli-keymap-reset.toml");
        fixture.events.truncate(28);
        reply(&mut fixture, 27)[0] = 0xff;
        let output = reset(&scratch, &fixture, &backup);
        failed(&output, "may have changed");
        assert!(
            stderr(&output).contains("keymap restore"),
            "{}",
            stderr(&output)
        );
        assert_eq!(saved_backup(&backup), original_backup());
    }

    #[test]
    fn write_reset_followed_by_a_changed_layer_count_retains_the_backup_without_success() {
        let scratch = Scratch::new();
        let backup = scratch.path("before-reset.json");
        let mut fixture = fixture("cli-keymap-reset.toml");
        fixture.events.truncate(30);
        reply(&mut fixture, 29)[1] = 2;
        let output = reset(&scratch, &fixture, &backup);
        failed(&output, "may have changed");
        assert_eq!(saved_backup(&backup), original_backup());
    }

    #[test]
    fn write_reset_refuses_recording_before_opening_a_session() {
        let scratch = Scratch::new();
        let backup = scratch.path("before-reset.json");
        let recording = scratch.path("reset.toml");
        let mut session = fixture("cli-keymap-reset.toml");
        session.events.clear();
        let output = run(
            &scratch,
            &session,
            &[
                "keymap",
                "reset",
                "--backup",
                backup.to_str().unwrap(),
                "--record",
                recording.to_str().unwrap(),
            ],
        );
        failed(&output, "--record");
        assert!(!backup.exists());
        assert!(!recording.exists());
    }

    #[test]
    fn write_set_refuses_invalid_or_unused_matrix_positions_without_reports() {
        for position in ["6,0", "0,19", "0,1", "0", "x,y"] {
            let scratch = Scratch::new();
            let backup = scratch.path("before.json");
            let mut fixture = fixture("cli-keymap-set.toml");
            fixture.events.clear();
            let output = run(
                &scratch,
                &fixture,
                &[
                    "keymap",
                    "set",
                    "0",
                    position,
                    "4",
                    "--backup",
                    backup.to_str().unwrap(),
                ],
            );
            let expected = if matches!(position, "0" | "x,y") {
                "ROW,COLUMN"
            } else {
                "physical key"
            };
            failed(&output, expected);
            assert!(!backup.exists());
        }
    }

    #[test]
    fn write_set_refuses_out_of_range_keycodes_without_reports() {
        for keycode in ["65536", "0x10000", "invalid"] {
            let scratch = Scratch::new();
            let backup = scratch.path("before.json");
            let mut fixture = fixture("cli-keymap-set.toml");
            fixture.events.clear();
            let output = run(
                &scratch,
                &fixture,
                &[
                    "keymap",
                    "set",
                    "0",
                    "2,1",
                    keycode,
                    "--backup",
                    backup.to_str().unwrap(),
                ],
            );
            failed(&output, "16-bit");
            assert!(!backup.exists());
        }
    }

    #[test]
    fn write_set_refuses_an_unavailable_layer_without_creating_a_backup() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let output = run(
            &scratch,
            &capture_only("cli-keymap-set.toml"),
            &[
                "keymap",
                "set",
                "1",
                "2,1",
                "4",
                "--backup",
                backup.to_str().unwrap(),
            ],
        );
        failed(&output, "layer 1");
        assert!(!backup.exists());
    }

    fn exchange(request: &[u8], reply: &[u8]) -> [Event; 2] {
        let report = |prefix: &[u8]| {
            let mut bytes = vec![0; 32];
            bytes[..prefix.len()].copy_from_slice(prefix);
            bytes
        };
        [
            Event::Write {
                data: report(request),
            },
            Event::Read {
                data: report(reply),
            },
        ]
    }

    fn two_layer_capture() -> Fixture {
        let mut session = fixture("cli-keymap.toml");
        let options = fixture("cli-keymap-set.toml").events[2..4].to_vec();
        session.events.splice(2..2, options);
        for (layer, clockwise, counter_clockwise) in [
            (0, [0x11, 0x11], [0x22, 0x22]),
            (1, [0x33, 0x33], [0x44, 0x44]),
        ] {
            session.events.extend(exchange(
                &[0x14, layer, 0, 1],
                &[0x14, layer, 0, 1, clockwise[0], clockwise[1]],
            ));
            session.events.extend(exchange(
                &[0x14, layer, 0, 0],
                &[
                    0x14,
                    layer,
                    0,
                    0,
                    counter_clockwise[0],
                    counter_clockwise[1],
                ],
            ));
        }
        session
    }

    #[test]
    fn write_set_captures_all_layers_and_both_directions_before_editing_layer_one() {
        let scratch = Scratch::new();
        let backup = scratch.path("two-layers.json");
        let mut session = two_layer_capture();
        let mut after = session.clone();
        // Offset 306 is layer 1, row 2, column 1, in the chunk starting at byte 280.
        reply(&mut after, 25)[31] = 0x42;
        session
            .events
            .extend(exchange(&[5, 1, 2, 1, 0, 0x42], &[5, 1, 2, 1, 0, 0x42]));
        session
            .events
            .extend(exchange(&[4, 1, 2, 1], &[4, 1, 2, 1, 0, 0x42]));
        session.events.extend(after.events);
        let output = run(
            &scratch,
            &session,
            &[
                "keymap",
                "set",
                "1",
                "2,1",
                "0x0042",
                "--backup",
                backup.to_str().unwrap(),
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let saved = saved_backup(&backup);
        assert_eq!(saved["matrix"].as_array().unwrap().len(), 228);
        assert_eq!(saved["matrix"][39], 0x0014);
        assert_eq!(saved["matrix"][153], 4);
        assert_eq!(
            saved["encoders"],
            json!([
                {"clockwise": 0x1111, "counter_clockwise": 0x2222},
                {"clockwise": 0x3333, "counter_clockwise": 0x4444},
            ])
        );
    }

    #[test]
    fn write_commands_refuse_recording_without_changing_protected_backup_files() {
        for operation in ["set", "restore"] {
            for destination in ["before.json", "source.json", "elsewhere.toml"] {
                let scratch = Scratch::new();
                let source = scratch.source(&original_backup());
                let before = scratch.path("before.json");
                std::fs::write(&before, "preserve this safety backup").unwrap();
                let recording = scratch.path(destination);
                let mut session = fixture("cli-keymap-set.toml");
                session.events.clear();
                // Keep an explicit fixture even on invalid-argument subprocess paths.
                // The parser-only unit test separately pins the mutation-specific conflict.
                let output = if operation == "set" {
                    run(
                        &scratch,
                        &session,
                        &[
                            "keymap",
                            "set",
                            "0",
                            "2,1",
                            "4",
                            "--backup",
                            before.to_str().unwrap(),
                            "--record",
                            recording.to_str().unwrap(),
                        ],
                    )
                } else {
                    run(
                        &scratch,
                        &session,
                        &[
                            "keymap",
                            "restore",
                            source.to_str().unwrap(),
                            "--backup",
                            before.to_str().unwrap(),
                            "--record",
                            recording.to_str().unwrap(),
                        ],
                    )
                };
                failed(&output, "--record");
                assert_eq!(output.status.code(), Some(2));
                assert_eq!(
                    std::fs::read_to_string(&before).unwrap(),
                    "preserve this safety backup"
                );
                assert_eq!(saved_backup(&source), original_backup());
                assert!(!scratch.path("elsewhere.toml").exists());
            }
        }
    }

    fn set_key(scratch: &Scratch, fixture: &Fixture, backup: &Path) -> Output {
        run(
            scratch,
            fixture,
            &[
                "keymap",
                "set",
                "0",
                "2,1",
                "4",
                "--backup",
                backup.to_str().unwrap(),
            ],
        )
    }

    #[test]
    fn write_refusal_message_names_the_only_supported_cable_keyboard() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let mut fixture = fixture("cli-keymap-set.toml");
        fixture.device.product_id = 0xd030;
        fixture.events.clear();
        let output = set_key(&scratch, &fixture, &backup);
        assert_eq!(
            stderr(&output),
            "openkeychron: keymap writes require the V5 Max ISO (3434:0951) on a USB cable; \
             receiver and unverified-model writes are refused (got receiver 3434:d030)\n"
        );
    }

    #[test]
    fn write_failure_after_the_backup_names_it_and_the_restore_command() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let mut fixture = fixture("cli-keymap-set.toml");
        reply(&mut fixture, 35)[5] = 0x2a;
        let output = set_key(&scratch, &fixture, &backup);
        let backup = backup.display();
        assert_eq!(
            stderr(&output),
            format!(
                "openkeychron: complete matrix/encoder keymap verification did not match the \
                 expected state; keymap state may have changed, and complete restoration was \
                 not confirmed; pre-change matrix/encoder backup retained at {backup}; undo \
                 with `openkeychron keymap restore {backup} --backup <NEW_FILE>`\n"
            )
        );
    }

    #[test]
    fn write_success_text_names_the_backup_and_the_undo_command() {
        let scratch = Scratch::new();
        let backup = scratch.path("before.json");
        let output = set_key(&scratch, &fixture("cli-keymap-set.toml"), &backup);
        let backup = backup.display();
        assert_eq!(
            stdout(&output),
            format!(
                "Keymap set verified against the complete matrix/encoder snapshot.\n\
                 Pre-change backup: {backup}\n\
                 Undo with: openkeychron keymap restore {backup} --backup <NEW_FILE>\n"
            )
        );
    }
}
