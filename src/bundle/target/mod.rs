use serde::{Deserialize, Serialize};

// [spec:kbdgen:req:bundle.structure.targets]
#[derive(Debug, Default)]
pub struct Targets {
    pub windows: Option<Windows>,
    pub macos: Option<MacOS>,
    pub ios: Option<iOS>,
    pub chromeos: Option<ChromeOS>,
    pub android: Option<Android>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Windows {
    pub(crate) app_name: String,
    pub(crate) version: String,
    pub(crate) url: String,
    pub(crate) uuid: String,
    pub(crate) build: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacOS {
    pub(crate) code_sign_id: String,
    pub(crate) package_id: String,
    pub(crate) bundle_name: String,
    pub(crate) version: String,
    pub(crate) build: String,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct iOS {
    #[serde(default)]
    pub(crate) code_sign_id: Option<String>,
    #[serde(default)]
    pub(crate) team_id: Option<String>,
    #[serde(default)]
    pub(crate) provisioning_profile_id: Option<String>,
    pub(crate) package_id: String,
    pub(crate) bundle_name: String,
    pub(crate) sentry_dsn: Option<String>,
    pub(crate) version: String,
    pub(crate) build: usize,

    #[serde(default)]
    /// https://docs.fastlane.tools/app-store-connect-api/#using-fastlane-api-key-json-file
    pub(crate) app_store_key_json: Option<String>,
    #[serde(default)]
    pub(crate) match_git_url: Option<String>,
    #[serde(default)]
    pub(crate) match_password: Option<String>,
    #[serde(default)]
    pub(crate) fastlane_user: Option<String>,
    #[serde(default)]
    pub(crate) fastlane_password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeOS {
    pub(crate) app_id: String,
    pub(crate) build: String,
    pub(crate) version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Android {
    #[serde(default)]
    pub(crate) key_store: Option<String>,
    #[serde(default)]
    pub(crate) key_alias: Option<String>,
    #[serde(default)]
    pub(crate) play_store_account: Option<String>,
    #[serde(default)]
    pub(crate) play_store_p12: Option<String>,
    #[serde(default)]
    pub(crate) store_password: Option<String>,
    #[serde(default)]
    pub(crate) key_password: Option<String>,
    pub(crate) package_id: String,
    pub(crate) build: usize,
    pub(crate) version: String,
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::{Mutex, MutexGuard};

    use crate::bundle::{Error, KbdgenBundle, fixture, read_kbdgen_bundle};

    const WINDOWS: &str = "appName: App\nversion: 1.0\nurl: https://example.com\nuuid: 1234\nbuild: 7\nextra: ignored\n";
    const MACOS: &str =
        "codeSignId: Sign\npackageId: no.example\nbundleName: Bundle\nversion: 2.0\nbuild: 3\n";
    const IOS: &str = "packageId: no.example.ios\nbundleName: Bundle\nversion: \"1.0\"\nbuild: 4\n";
    const CHROMEOS: &str = "appId: abc\nbuild: 5\nversion: 1.2.3\n";
    const ANDROID: &str = "packageId: no.example.android\nversion: \"1.0\"\nbuild: 6\n";

    fn load_with_targets(targets: &[(&str, &str)]) -> Result<KbdgenBundle, Error> {
        let root = tempfile::tempdir().unwrap();
        let path = fixture::write_bundle(root.path(), "sme", &[], targets, &[]);
        read_kbdgen_bundle(&path)
    }

    /// Serialises environment mutation within this test binary and restores
    /// every touched variable on drop, so a run under `cargo test` (shared
    /// process) neither races nor leaks values into other tests.
    struct EnvGuard {
        saved: Vec<(&'static str, Option<OsString>)>,
        _lock: MutexGuard<'static, ()>,
    }

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    impl EnvGuard {
        fn new() -> Self {
            EnvGuard {
                saved: Vec::new(),
                _lock: ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner()),
            }
        }

        fn save(&mut self, name: &'static str) {
            if !self.saved.iter().any(|(saved, _)| *saved == name) {
                self.saved.push((name, std::env::var_os(name)));
            }
        }

        fn set(&mut self, name: &'static str, value: &str) {
            self.save(name);
            // SAFETY: ENV_LOCK is held, and no other code in this crate
            // writes the environment.
            unsafe { std::env::set_var(name, value) };
        }

        fn remove(&mut self, name: &'static str) {
            self.save(name);
            // SAFETY: as in `set`.
            unsafe { std::env::remove_var(name) };
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in self.saved.drain(..) {
                // SAFETY: as in `set`; the lock is released after this body.
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
    }

    // [spec:kbdgen:req:bundle.structure.targets/test]
    #[test]
    fn recognised_stems_load_and_unknown_are_ignored() {
        let _env = EnvGuard::new();
        let bundle = load_with_targets(&[
            ("windows", WINDOWS),
            ("macos", MACOS),
            ("ios", IOS),
            ("chromeos", CHROMEOS),
            ("android", ANDROID),
            ("linux", "not: [valid"),
            ("ipados", "- not a mapping\n"),
        ])
        .unwrap();
        let targets = bundle.targets;

        let windows = targets.windows.unwrap();
        assert_eq!(windows.app_name, "App");
        assert_eq!(windows.version, "1.0");
        assert_eq!(windows.uuid, "1234");
        assert_eq!(windows.build, "7");
        let macos = targets.macos.unwrap();
        assert_eq!(macos.version, "2.0");
        assert_eq!(macos.build, "3");
        let ios = targets.ios.unwrap();
        assert_eq!(ios.package_id, "no.example.ios");
        assert_eq!(ios.build, 4);
        let chromeos = targets.chromeos.unwrap();
        assert_eq!(chromeos.app_id, "abc");
        assert_eq!(chromeos.build, "5");
        assert_eq!(chromeos.version, "1.2.3");
        let android = targets.android.unwrap();
        assert_eq!(android.package_id, "no.example.android");
        assert_eq!(android.build, 6);
    }

    // [spec:kbdgen:req:bundle.structure.targets/test]
    #[test]
    fn absent_target_files_leave_targets_unconfigured() {
        let bundle = load_with_targets(&[("windows", WINDOWS)]).unwrap();

        assert!(bundle.targets.windows.is_some());
        assert!(bundle.targets.macos.is_none());
        assert!(bundle.targets.ios.is_none());
        assert!(bundle.targets.chromeos.is_none());
        assert!(bundle.targets.android.is_none());
    }

    // [spec:kbdgen:req:bundle.structure.targets/test]
    #[test]
    fn target_parse_failure_aborts_loading() {
        let _env = EnvGuard::new();
        for (stem, yaml) in [
            ("windows", WINDOWS.replace("uuid: 1234\n", "")),
            ("macos", MACOS.replace("bundleName: Bundle\n", "")),
            ("chromeos", CHROMEOS.replace("appId: abc\n", "")),
            ("ios", IOS.replace("build: 4", "build: -1")),
            (
                "android",
                ANDROID.replace("packageId: no.example.android\n", ""),
            ),
        ] {
            match load_with_targets(&[(stem, &yaml)]) {
                Err(Error::Yaml(path, _)) => {
                    assert!(path.ends_with(format!("targets/{stem}.yaml")), "{stem}");
                }
                other => panic!("{stem}: expected a YAML error, got {other:?}"),
            }
        }
    }

    // [spec:kbdgen:req:bundle.structure.targets/test]
    #[test]
    fn ios_and_android_strings_must_be_yaml_strings() {
        let _env = EnvGuard::new();
        for (stem, yaml) in [
            ("ios", IOS.replace("version: \"1.0\"", "version: 1.0")),
            (
                "android",
                ANDROID.replace("version: \"1.0\"", "version: 1.0"),
            ),
        ] {
            assert!(
                matches!(load_with_targets(&[(stem, &yaml)]), Err(Error::Yaml(..))),
                "{stem}"
            );
        }
    }

    // [spec:kbdgen:req:bundle.structure.targets/test]
    #[test]
    #[should_panic]
    fn non_mapping_ios_target_panics() {
        let _env = EnvGuard::new();
        let _ = load_with_targets(&[("ios", "- packageId\n")]);
    }

    // [spec:kbdgen:req:bundle.structure.targets.env/test]
    #[test]
    fn ios_env_variables_replace_file_fields() {
        let mut env = EnvGuard::new();
        env.set("MATCH_GIT_URL", "git@example.com:certs");
        env.set("TEAM_ID", "");
        env.set("CODE_SIGN_ID", "12345");
        env.remove("MATCH_PASSWORD");
        let ios = format!(
            "{IOS}matchGitUrl: file-url\nteamId: file-team\nmatchPassword: file-password\n"
        );

        let ios = load_with_targets(&[("ios", &ios)])
            .unwrap()
            .targets
            .ios
            .unwrap();

        assert_eq!(ios.match_git_url.as_deref(), Some("git@example.com:certs"));
        assert_eq!(ios.team_id.as_deref(), Some(""));
        assert_eq!(ios.code_sign_id.as_deref(), Some("12345"));
        assert_eq!(ios.match_password.as_deref(), Some("file-password"));
    }

    // [spec:kbdgen:req:bundle.structure.targets.env/test]
    #[test]
    fn android_env_variables_replace_file_fields() {
        let mut env = EnvGuard::new();
        env.set("ANDROID_KEYALIAS", "env-alias");
        env.set("KEY_PW", "0042");
        env.remove("STORE_PW");
        env.remove("ANDROID_KEYSTORE");
        let android = format!("{ANDROID}keyAlias: file-alias\nstorePassword: file-store\n");

        let android = load_with_targets(&[("android", &android)])
            .unwrap()
            .targets
            .android
            .unwrap();

        assert_eq!(android.key_alias.as_deref(), Some("env-alias"));
        assert_eq!(android.key_password.as_deref(), Some("0042"));
        assert_eq!(android.store_password.as_deref(), Some("file-store"));
        assert_eq!(android.key_store, None);
    }

    // [spec:kbdgen:req:bundle.structure.targets.env/test]
    #[test]
    fn env_variables_need_an_existing_target_file() {
        let mut env = EnvGuard::new();
        env.set("MATCH_GIT_URL", "git@example.com:certs");
        env.set("ANDROID_KEYALIAS", "env-alias");

        let targets = load_with_targets(&[]).unwrap().targets;

        assert!(targets.ios.is_none());
        assert!(targets.android.is_none());
    }
}
