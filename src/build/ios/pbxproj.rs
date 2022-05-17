use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[nova::newtype(serde)]
pub type ObjectId = String;

impl ObjectId {
    pub fn new_random() -> Self {
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

    pub fn add_ref_to_group() {
        println!("ADD_REF_TO_GROUP");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pbxproj {
    pub classes: serde_json::Value,
    pub object_version: String,
    pub archive_version: String,
    pub objects: IndexMap<ObjectId, Object>,
    pub mainGroup: IndexMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct PlistFile {
    #[serde(rename = "lastKnownFileType")]
    last_known_file_type: String,
    name: String,
    path: String,
    #[serde(rename = "sourceTree")]
    source_tree: String,
}

#[derive(Serialize, Deserialize)]
pub struct PbxGroup {
    children: Vec<String>,
    isa: String,
    path: String,
    #[serde(rename = "sourceTree")]
    source_tree: String,
}

impl Pbxproj {
    pub fn from_path(path: &PathBuf) -> Self {
        convert_pbxproj_to_json(path)
    }

    pub fn create_plist_file(&mut self, relative_plist_path: &PathBuf) -> ObjectId {
        let temp = PlistFile {
            last_known_file_type: "text.plist.xml".to_string(),
            name: relative_plist_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            path: relative_plist_path.to_str().unwrap().to_string(),
            source_tree: "<group>".to_string(),
        };

        let object = ObjectId::new_random();

        self.objects.insert(
            object.clone(),
            Object::FileReference(serde_json::json!(temp)),
        );

        return object;
    }

    // WTF IS HAPPENING IN THE PYTHON?
    // create a self.main_group??
    pub fn add_path(&self, path: &PathBuf) {
        println!("ADD_PATH: {:?}", path);
        let path_list: Vec<String> = path
            .to_str()
            .unwrap()
            .to_string()
            .split("/")
            .map(|x| x.to_string())
            .collect();

        // let mut target = self.mainGroup;

        // for path_name in path_list {
        //     for child in target.children {
        //         if child.get("path").cmp(&path_name) == Ordering::Equal {
        //             target = child;
        //             break;
        //         } else {
        //             let key = ObjectId::new_random();

        //             let value = PbxGroup {
        //                 children: vec![],
        //                 isa: "PBXGroup".to_string(),
        //                 path: path_name,
        //                 source_tree: "<group>".to_string(),
        //             };

        //             self.objects.insert(key, Object::Group(serde_json::json!(value)));
        //             target["children"].push(serde_json::json!(key));
        //             target = self.objects.get(&key);
        //         }
        //     }
        // }
    }
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
