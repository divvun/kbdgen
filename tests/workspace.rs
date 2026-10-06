//! The workspace's shape and its members' manifests, as `cargo metadata`
//! reports them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const DEFAULT_MEMBERS: &[&str] = &["kbdgen", "kbd-model", "kbd-engine", "kbd-ldml"];

struct Workspace {
    root: PathBuf,
    metadata: Value,
}

impl Workspace {
    fn load() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let output = Command::new(env!("CARGO"))
            .args(["metadata", "--format-version", "1", "--no-deps"])
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml"))
            .output()
            .expect("cargo metadata runs");
        assert!(
            output.status.success(),
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata = serde_json::from_slice(&output.stdout).expect("cargo metadata emits JSON");
        Workspace { root, metadata }
    }

    fn packages(&self) -> &[Value] {
        self.metadata["packages"].as_array().expect("packages")
    }

    fn package(&self, name: &str) -> &Value {
        self.packages()
            .iter()
            .find(|package| package["name"] == name)
            .unwrap_or_else(|| panic!("package {name} is a workspace member"))
    }

    fn names_of(&self, ids_key: &str) -> BTreeSet<String> {
        let ids = self.metadata[ids_key].as_array().expect(ids_key);
        self.packages()
            .iter()
            .filter(|package| ids.contains(&package["id"]))
            .map(|package| package["name"].as_str().expect("name").to_owned())
            .collect()
    }
}

/// Dependencies other than dev-dependencies, by name.
fn linked_dependencies(package: &Value) -> Vec<&Value> {
    package["dependencies"]
        .as_array()
        .expect("dependencies")
        .iter()
        .filter(|dependency| dependency["kind"] != "dev")
        .collect()
}

fn dependency<'a>(package: &'a Value, name: &str) -> &'a Value {
    linked_dependencies(package)
        .into_iter()
        .find(|dependency| dependency["name"] == name)
        .unwrap_or_else(|| panic!("{} depends on {name}", package["name"]))
}

fn dependency_names(package: &Value) -> BTreeSet<&str> {
    linked_dependencies(package)
        .into_iter()
        .map(|dependency| dependency["name"].as_str().expect("name"))
        .collect()
}

fn strings(value: &Value) -> BTreeSet<&str> {
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|item| item.as_str().expect("string"))
        .collect()
}

fn assert_no_default_features(dependency: &Value, features: &[&str]) {
    assert_eq!(
        dependency["uses_default_features"], false,
        "{} is used without default features",
        dependency["name"]
    );
    assert_eq!(
        strings(&dependency["features"]),
        features.iter().copied().collect(),
        "{} features",
        dependency["name"]
    );
}

fn crate_dirs(root: &Path) -> BTreeSet<String> {
    std::fs::read_dir(root.join("crates"))
        .expect("crates/ exists")
        .map(|entry| entry.expect("crates/ entry").path())
        .filter(|path| path.join("Cargo.toml").is_file())
        .map(|path| {
            path.file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

// [spec:kbdgen:def:ldml.crate.layout+1/test]
#[test]
fn workspace_has_kbdgen_and_crate_members() {
    let workspace = Workspace::load();

    let members = workspace.names_of("workspace_members");
    let mut expected = crate_dirs(&workspace.root);
    expected.insert("kbdgen".to_owned());
    assert_eq!(members, expected, "members are kbdgen and crates/*");
    for name in DEFAULT_MEMBERS {
        assert!(
            workspace.root.join("crates").join(name).is_dir() || *name == "kbdgen",
            "{name} lives in crates/{name}"
        );
    }

    assert_eq!(
        workspace.names_of("workspace_default_members"),
        DEFAULT_MEMBERS
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
    );

    for package in workspace.packages() {
        assert_eq!(package["edition"], "2024", "{} edition", package["name"]);
    }

    for name in ["kbd-model", "kbd-engine", "kbd-ldml"] {
        let kinds: BTreeSet<&str> = workspace.package(name)["targets"]
            .as_array()
            .expect("targets")
            .iter()
            .filter(|target| {
                target["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind != "test"))
            })
            .flat_map(|target| strings(&target["kind"]))
            .collect();
        assert_eq!(kinds, BTreeSet::from(["lib"]), "{name} is a plain library");
    }

    let manifest =
        std::fs::read_to_string(workspace.root.join("Cargo.toml")).expect("root manifest");
    assert!(
        manifest
            .lines()
            .any(|line| line.trim() == r#"resolver = "3""#),
        "the workspace uses resolver 3"
    );
}

// [spec:kbdgen:req:ldml.crate.model/test]
#[test]
fn model_depends_only_on_serde_and_postcard() {
    let workspace = Workspace::load();
    let model = workspace.package("kbd-model");

    assert_eq!(
        dependency_names(model),
        BTreeSet::from(["postcard", "serde"])
    );
    assert_no_default_features(dependency(model, "serde"), &["alloc", "derive"]);
    assert_no_default_features(dependency(model, "postcard"), &["alloc"]);
}

// [spec:kbdgen:req:ldml.crate.engine+1/test]
#[test]
fn engine_depends_on_model_and_gated_icu_normalizer() {
    let workspace = Workspace::load();
    let engine = workspace.package("kbd-engine");

    assert_eq!(
        dependency_names(engine),
        BTreeSet::from(["icu_normalizer", "kbd-model"])
    );
    assert!(dependency(engine, "kbd-model")["path"].is_string());

    let icu = dependency(engine, "icu_normalizer");
    assert_no_default_features(icu, &["compiled_data"]);
    assert_eq!(icu["optional"], true, "icu_normalizer is behind a feature");
    assert!(
        icu["req"].as_str().expect("req").starts_with("^2"),
        "icu_normalizer is 2.x"
    );

    let features = &engine["features"];
    assert!(strings(&features["default"]).contains("normalization"));
    assert_eq!(
        strings(&features["normalization"]),
        BTreeSet::from(["dep:icu_normalizer"])
    );
}

// [spec:kbdgen:req:ldml.crate.ldml/test]
#[test]
fn ldml_depends_on_model_icu_normalizer_and_xmlem() {
    let workspace = Workspace::load();
    let ldml = workspace.package("kbd-ldml");

    assert_eq!(
        dependency_names(ldml),
        BTreeSet::from(["icu_normalizer", "kbd-model", "xmlem"])
    );
    assert!(dependency(ldml, "kbd-model")["path"].is_string());
    assert_eq!(
        dependency(ldml, "xmlem")["req"],
        "^0.3.3",
        "the workspace's xmlem"
    );
    let kbdgen = workspace.package("kbdgen");
    assert_eq!(
        dependency(kbdgen, "xmlem")["req"],
        dependency(ldml, "xmlem")["req"]
    );
    assert!(
        dependency(ldml, "icu_normalizer")["req"]
            .as_str()
            .expect("req")
            .starts_with("^2"),
        "icu_normalizer is 2.x"
    );
}

// [spec:kbdgen:req:ldml.crate.kbdgen/test]
#[test]
fn kbdgen_depends_on_crates_by_path() {
    let workspace = Workspace::load();
    let kbdgen = workspace.package("kbdgen");

    for name in ["kbd-model", "kbd-engine", "kbd-ldml"] {
        let path = dependency(kbdgen, name)["path"]
            .as_str()
            .unwrap_or_else(|| panic!("kbdgen depends on {name} by path"));
        assert_eq!(Path::new(path), workspace.root.join("crates").join(name));
    }
}

// [spec:kbdgen:req:ldml.crate.tsf+1/test]
// [spec:kbdgen:req:tsf.component.crate/test]
#[test]
fn tsf_is_windows_cdylib_on_engine_and_windows() {
    let workspace = Workspace::load();
    let tsf = workspace.package("kbd-tsf");

    let library = tsf["targets"]
        .as_array()
        .expect("targets")
        .iter()
        .find(|target| target["name"] == "kbd_tsf")
        .expect("kbd-tsf has a library");
    assert_eq!(strings(&library["crate_types"]), BTreeSet::from(["cdylib"]));
    assert!(
        !workspace
            .names_of("workspace_default_members")
            .contains("kbd-tsf"),
        "a plain cargo build never compiles the text service"
    );

    assert_eq!(
        dependency_names(tsf),
        BTreeSet::from(["kbd-engine", "kbd-model", "windows", "windows-core"])
    );
    for name in ["kbd-engine", "kbd-model"] {
        let dependency = dependency(tsf, name);
        assert!(dependency["path"].is_string(), "{name} by path");
        assert_eq!(dependency["uses_default_features"], true, "{name} defaults");
    }
    for name in ["windows", "windows-core"] {
        let dependency = dependency(tsf, name);
        assert_eq!(
            dependency["target"], "cfg(windows)",
            "{name} only on Windows"
        );
        assert!(
            dependency["req"]
                .as_str()
                .expect("req")
                .starts_with("^0.62"),
            "{name} is 0.62.x"
        );
    }
}
