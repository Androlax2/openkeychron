//! The Linux packages ship the one udev rule kept in `packaging/linux/`, so the rule a user
//! reads in the repository is the rule the package installs.

#[cfg(test)]
mod tests {
    use std::path::Path;

    const RULE_INSTALL_PATH: &str = "/usr/lib/udev/rules.d/70-openkeychron.rules";

    fn tauri_config() -> serde_json::Value {
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap()
    }

    #[test]
    fn tauri_build_makes_a_deb_and_an_appimage() {
        let bundle = &tauri_config()["bundle"];
        assert_eq!(bundle["active"], serde_json::json!(true));
        assert_eq!(bundle["targets"], serde_json::json!(["deb", "appimage"]));
    }

    #[test]
    fn the_deb_installs_the_udev_rule_from_packaging_linux() {
        let rule_source = &tauri_config()["bundle"]["linux"]["deb"]["files"][RULE_INSTALL_PATH];
        let rule_source = rule_source
            .as_str()
            .unwrap_or_else(|| panic!("the deb maps nothing to {RULE_INSTALL_PATH}"));

        // Tauri resolves bundle file sources against the directory holding tauri.conf.json.
        let resolved = Path::new(env!("CARGO_MANIFEST_DIR")).join(rule_source);
        let repository_rule = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packaging/linux/70-openkeychron.rules");
        assert_eq!(
            resolved.canonicalize().unwrap(),
            repository_rule.canonicalize().unwrap()
        );
    }
}
