use std::path::{Path, PathBuf};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbxProject {
    attributes: serde_json::Value,
    build_configuration_list: ObjectId,
    compatibility_version: String,
    development_region: String,
    has_scanned_for_encodings: String,
    known_regions: serde_json::Value,
    main_group: ObjectId,
    product_ref_group: ObjectId,
    project_dir_path: String,
    project_root: String,
    targets: Vec<ObjectId>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbxGroup {
    children: Vec<ObjectId>,
    #[serde(rename = "sourceTree")]
    source_tree: String,
    path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pbxproj {
    pub classes: serde_json::Value,
    pub object_version: String,
    pub archive_version: String,
    pub objects: IndexMap<ObjectId, Object>,
}

impl Pbxproj {
    pub fn from_path(path: &PathBuf) -> Self {
        convert_pbxproj_to_json(path)
    }

    pub fn project(&self) -> Option<PbxProject> {
        for object in self.objects.values() {
            if let Object::Project(project) = object {
                return Some(project.clone());
            }
        }
        None
    }

    pub fn group(&self, object_id: &ObjectId) -> Option<&PbxGroup> {
        if let Some(Object::Group(main_group)) = self.objects.get(object_id) {
            return Some(main_group);
        }
        return None;
    }

    pub fn group_mut(&mut self, object_id: &ObjectId) -> Option<&mut PbxGroup> {
        if let Some(Object::Group(main_group)) = self.objects.get_mut(object_id) {
            return Some(main_group);
        }
        return None;
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

    pub fn add_path(&mut self, path: &PathBuf) {
        let path_names: Vec<String> = path
            .components()
            .map(|x| x.as_os_str().to_str().unwrap().to_string())
            .collect();

        let mut target = self.project().unwrap().main_group;

        'boop: for path_name in path_names {
            let children_references = &self.group(&target).unwrap().children;

            for child_reference in children_references {
                if let Some(child_group) = self.group(child_reference) {
                    if let Some(path) = &child_group.path {
                        if path == &path_name {
                            target = child_reference.clone();
                            continue 'boop;
                        }
                    }
                }
            }

            println!("Nothing exists create here");

            let id = ObjectId::new_random();

            let new_child = PbxGroup {
                children: vec![],
                path: Some(path_name.clone()),
                source_tree: "<group>".to_string(),
            };

            self.objects.insert(id.clone(), Object::Group(new_child));
            self.group_mut(&target).unwrap().children.push(id.clone());

            target = id;
        }
    }

    // TODO: almost identical to add_path
    pub fn add_ref_to_group(&mut self, object_id: &ObjectId, group: &PathBuf) {
        let group_names: Vec<String> = group
            .components()
            .map(|x| x.as_os_str().to_str().unwrap().to_string())
            .collect();

        let mut object = self.project().unwrap().main_group;

        'boop: for group_name in group_names {
            let children_references = &self.group(&object).unwrap().children;

            for child_reference in children_references {
                if let Some(child_group) = self.group(child_reference) {
                    if let Some(path) = &child_group.path {
                        if path == &group_name {
                            object = child_reference.clone();
                            continue 'boop;
                        }
                    }
                }
            }
            let id = ObjectId::new_random();

            let new_child = PbxGroup {
                children: vec![],
                path: Some(group_name.clone()),
                source_tree: "<group>".to_string(),
            };

            self.objects.insert(id.clone(), Object::Group(new_child));
            self.group_mut(&object).unwrap().children.push(id.clone());

            object = id;
        }
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
    Group(PbxGroup),

    #[serde(rename = "PBXProject")]
    Project(PbxProject),

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
