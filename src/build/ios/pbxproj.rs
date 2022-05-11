use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[nova::newtype(serde)]
pub type ObjectId = String;

impl ObjectId {
    pub fn random() -> Self {
        use rand::Rng;
        const CHARSET: &[u8] = b"0123456789ABCDEF";
        const LEN: usize = 24;
        let mut rng = rand::thread_rng();

        let id: String = (0..LEN)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect();

        ObjectId(id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pbxproj {
    pub classes: serde_json::Value,
    pub object_version: String,
    pub archive_version: String,
    pub objects: IndexMap<ObjectId, Object>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "isa")]
pub enum Object {
    #[serde(rename = "PBXBuildFile")]
    BuildFile(BuildFile),

    #[serde(rename = "PBXFileReference")]
    FileReference(serde_json::Value),

    #[serde(rename = "PBXCopyFilesBuildPhase")]
    CopyFilesBuildPhase(serde_json::Value),

    #[serde(rename = "PBXGroup")]
    Group(serde_json::Value),

    #[serde(rename = "PBXProject")]
    Project(serde_json::Value),

    #[serde(rename = "XCConfigurationList")]
    ConfigurationList(serde_json::Value),

    #[serde(rename = "PBXSourcesBuildPhase")]
    SourcesBuildPhase(serde_json::Value),

    #[serde(rename = "PBXFrameworksBuildPhase")]
    FrameworksBuildPhase(serde_json::Value),

    #[serde(rename = "PBXResourcesBuildPhase")]
    ResourcesBuildPhase(serde_json::Value),

    #[serde(rename = "PBXTargetDependency")]
    TargetDependency(serde_json::Value),

    #[serde(rename = "PBXVariantGroup")]
    VariantGroup(serde_json::Value),

    #[serde(rename = "PBXShellScriptBuildPhase")]
    ShellScriptBuildPhase(serde_json::Value),

    #[serde(rename = "PBXHeadersBuildPhase")]
    HeadersBuildPhase(serde_json::Value),

    #[serde(rename = "PBXNativeTarget")]
    NativeTarget(serde_json::Value),

    #[serde(rename = "XCBuildConfiguration")]
    BuildConfiguration(serde_json::Value),

    #[serde(rename = "PBXContainerItemProxy")]
    ContainerItemProxy(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildFile {
    pub file_ref: ObjectId,
}

pub fn convert_pbxproj_to_json(path: &Path) -> Pbxproj {
    let tempdir = tempfile::tempdir().unwrap();
    let pbxproj_path = tempdir.path().join("tmp.pbxproj");
    std::fs::copy(path, &pbxproj_path).unwrap();

    std::process::Command::new("plutil")
        .args(["-convert", "json"])
        .arg(&pbxproj_path)
        .status()
        .unwrap();

    let reader = std::fs::File::open(pbxproj_path).unwrap();
    serde_json::from_reader(reader).unwrap()
}
