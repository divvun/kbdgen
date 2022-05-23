use std::{
    borrow::BorrowMut,
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PBXFileReference {
    #[serde(flatten)]
    fields: IndexMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbxGroup {
    children: Vec<ObjectId>,
    #[serde(rename = "sourceTree")]
    source_tree: String,
    path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTarget {
    #[serde(rename = "buildConfigurationList")]
    build_configuration_list: ObjectId,
    #[serde(rename = "productReference")]
    product_reference: ObjectId,
    #[serde(rename = "productName")]
    product_name: String,
    #[serde(rename = "buildPhases")]
    build_phases: Vec<ObjectId>,
    dependencies: serde_json::Value,
    name: String,
    #[serde(rename = "buildRules")]
    build_rules: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationList {
    #[serde(rename = "buildConfigurations")]
    build_configurations: Vec<ObjectId>,
    #[serde(rename = "defaultConfigurationIsVisible")]
    default_configuration_is_visible: String,
    #[serde(rename = "defaultConfigurationName")]
    default_configuration_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfiguration {
    #[serde(rename = "buildSettings")]
    build_settings: IndexMap<String, serde_json::Value>,
    name: String,
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

    pub fn project_mut(&mut self) -> Option<&mut PbxProject> {
        for object in self.objects.values_mut() {
            if let Object::Project(project) = object {
                return Some(project);
            }
        }
        None
    }

    pub fn add_target(&mut self, object_id: &ObjectId) {
        self.project_mut().unwrap().targets.push(object_id.clone());
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

    pub fn file_reference_by_id(&self, object_id: &ObjectId) -> Option<&PBXFileReference> {
        for object in &self.objects {
            if let (id, Object::FileReference(file_reference)) = object {
                if id == object_id {
                    return Some(file_reference);
                }
            }
        }
        None
    }

    pub fn native_target_by_name(&self, name: &str) -> Option<&NativeTarget> {
        for object in self.objects.values() {
            if let Object::NativeTarget(native_target) = object {
                if native_target.name == name {
                    return Some(native_target);
                }
            }
        }
        None
    }

    pub fn configuration_list_by_id(&self, object_id: &ObjectId) -> Option<&ConfigurationList> {
        for object in &self.objects {
            if let (id, Object::ConfigurationList(configuration_list)) = object {
                if id == object_id {
                    return Some(configuration_list);
                }
            }
        }
        None
    }

    pub fn configuration_by_id(&self, object_id: &ObjectId) -> Option<&BuildConfiguration> {
        for object in &self.objects {
            if let (id, Object::BuildConfiguration(configuration_list)) = object {
                if id == object_id {
                    return Some(configuration_list);
                }
            }
        }
        None
    }

    pub fn create_plist_file(&mut self, relative_plist_path: &PathBuf) -> ObjectId {
        let mut plist_file_fields: IndexMap<String, String> = IndexMap::new();

        plist_file_fields.insert(
            "lastKnownFileType".to_string(),
            "text.plist.xml".to_string(),
        );
        plist_file_fields.insert(
            "name".to_string(),
            relative_plist_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
        );
        plist_file_fields.insert(
            "path".to_string(),
            relative_plist_path.to_str().unwrap().to_string(),
        );
        plist_file_fields.insert("sourceTree".to_string(), "<group>".to_string());

        let object = ObjectId::new_random();

        self.objects.insert(
            object.clone(),
            Object::FileReference(PBXFileReference {
                fields: plist_file_fields,
            }),
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

    pub fn add_ref_to_group(&mut self, object_id: &ObjectId, group: &PathBuf) {
        let group_names: Vec<String> = group
            .components()
            .map(|x| x.as_os_str().to_str().unwrap().to_string())
            .collect();

        let mut object = self.project().unwrap().main_group;

        for group_name in group_names {
            let children_references = &self.group(&object).unwrap().children;

            for child_reference in children_references {
                if let Some(child_group) = self.group(child_reference) {
                    if let Some(child_path) = &child_group.path {
                        if child_path == &group_name {
                            object = child_reference.clone();
                            break;
                        }
                    }
                }
            }
        }

        self.group_mut(&object)
            .unwrap()
            .children
            .push(object_id.clone());
    }

    pub fn duplicate_target(
        &mut self,
        source_name: String,
        destination_name: String,
        plist_path: &PathBuf,
    ) {
        let mut new_native_target = match self.native_target_by_name(&source_name) {
            Some(native_target) => native_target.clone(),
            None => std::panic!(
                "No native target with name `{}` could be found in .pbxproj",
                source_name
            ),
        };

        let new_native_target_id = ObjectId::new_random();
        new_native_target.name = destination_name.clone();

        let new_configuration_list_id = ObjectId::new_random();
        let mut new_configuration_list = self
            .configuration_list_by_id(&new_native_target.build_configuration_list)
            .unwrap()
            .clone();

        // Create new build configurations
        let mut new_configuration_list_refs: Vec<ObjectId> = Vec::new();
        for build_configuration_id in &new_configuration_list.build_configurations {
            let new_build_configuration_id = ObjectId::new_random();
            let mut new_build_configuration = self
                .configuration_by_id(build_configuration_id)
                .unwrap()
                .clone();

            new_build_configuration.build_settings.insert(
                "INFOPLIST_FILE".to_string(),
                serde_json::Value::String(plist_path.to_str().unwrap().to_string().clone()),
            );
            new_build_configuration.build_settings.insert(
                "PRODUCT_NAME".to_string(),
                serde_json::Value::String(destination_name.clone()),
            );
            new_build_configuration.build_settings.insert(
                "CODE_SIGN_STYLE".to_string(),
                serde_json::Value::String("CODE_SIGN_STYLE".to_string()),
            );
            new_build_configuration.build_settings.insert(
                "PRODUCT_NAENABLE_BITCODEME".to_string(),
                serde_json::Value::String("ENABLE_BITCODE".to_string()),
            );

            new_configuration_list_refs.push(new_build_configuration_id.clone());
            // Add to the actual .pbxproj
            // Each new build configuration
            self.objects.insert(
                new_build_configuration_id.clone(),
                Object::BuildConfiguration(new_build_configuration),
            );
        }
        // Add all new build configuration references to the list
        new_configuration_list.build_configurations = new_configuration_list_refs;

        // add new configuration list reference to the new native target
        new_native_target.build_configuration_list = new_configuration_list_id.clone();

        // Appex
        let new_appex_id = ObjectId::new_random();
        let mut new_appex = self
            .file_reference_by_id(&new_native_target.product_reference)
            .unwrap()
            .clone();
        new_appex
            .fields
            .insert("path".to_string(), format!("{}.appex", destination_name));

        // Add new appex id to the new native target
        new_native_target.product_reference = new_appex_id.clone();

        // Add to the actual .pbxproj
        // The new appex
        self.objects
            .insert(new_appex_id.clone(), Object::FileReference(new_appex));
        // The new configuration list
        self.objects.insert(
            new_configuration_list_id,
            Object::ConfigurationList(new_configuration_list),
        );
        // The new native target
        self.objects.insert(
            new_native_target_id,
            Object::NativeTarget(new_native_target),
        );

        // Finishing up
        // TODO: No "Products" group exists, create one?
        // self.add_ref_to_group(&new_appex_id, &PathBuf::from_str("Products").unwrap());
        self.add_target(&new_appex_id);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "isa")]
pub enum Object {
    #[serde(rename = "PBXBuildFile")]
    BuildFile(BuildFile),

    #[serde(rename = "PBXFileReference")]
    FileReference(PBXFileReference),

    #[serde(rename = "PBXCopyFilesBuildPhase")]
    CopyFilesBuildPhase(serde_json::Value),

    #[serde(rename = "PBXGroup")]
    Group(PbxGroup),

    #[serde(rename = "PBXProject")]
    Project(PbxProject),

    #[serde(rename = "XCConfigurationList")]
    ConfigurationList(ConfigurationList),

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
    NativeTarget(NativeTarget),

    #[serde(rename = "XCBuildConfiguration")]
    BuildConfiguration(BuildConfiguration),

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
