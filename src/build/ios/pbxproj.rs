use std::{
    borrow::BorrowMut,
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    str::FromStr,
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::bundle::target;

#[nova::newtype(serde, display)]
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
#[serde(deny_unknown_fields)]
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
    targets: BTreeSet<ObjectId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PBXFileReference {
    file_encoding: Option<String>,
    include_in_index: Option<String>,
    last_known_file_type: Option<String>,
    explicit_file_type: Option<String>,
    name: Option<String>,
    path: String,
    source_tree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PbxGroup {
    children: BTreeSet<ObjectId>,
    source_tree: String,
    name: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct NativeTarget {
    product_type: Option<String>,
    build_configuration_list: ObjectId,
    product_reference: ObjectId,
    product_name: String,
    build_phases: BTreeSet<ObjectId>,
    dependencies: BTreeSet<ObjectId>,
    name: String,
    build_rules: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ConfigurationList {
    build_configurations: BTreeSet<ObjectId>,
    default_configuration_is_visible: String,
    default_configuration_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct BuildConfiguration {
    build_settings: IndexMap<String, serde_json::Value>,
    base_configuration_reference: Option<ObjectId>,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PBXTargetDependency {
    target: ObjectId,
    target_proxy: ObjectId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbxCopyFilesBuildPhase {
    #[serde(rename = "buildActionMask")]
    build_action_mask: String,
    #[serde(rename = "dstPath")]
    dst_path: String,
    #[serde(rename = "dstSubfolderSpec")]
    dst_subfolder_spec: String,
    files: BTreeSet<ObjectId>,
    name: Option<String>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    run_only_for_deployment_postprocessing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Pbxproj {
    pub classes: serde_json::Value,
    pub object_version: String,
    pub archive_version: String,
    pub objects: BTreeMap<ObjectId, Object>,
    pub root_object: ObjectId,
}

macro_rules! iter_object {
    ($self:path, $ty:ident) => {
        $self.objects.iter().filter_map(|(k, v)| {
            if let Object::$ty(x) = v {
                Some((k, x))
            } else {
                None
            }
        })
    };
}

impl Pbxproj {
    pub fn from_path(path: &Path) -> Self {
        convert_pbxproj_to_json(path)
    }

    pub fn project(&self) -> Option<&PbxProject> {
        if let Some(Object::Project(project)) = self.objects.get(&self.root_object) {
            return Some(project);
        }
        None
    }

    pub fn project_mut(&mut self) -> Option<&mut PbxProject> {
        if let Some(Object::Project(project)) = self.objects.get_mut(&self.root_object) {
            return Some(project);
        }
        None
    }

    pub fn add_target(&mut self, object_id: &ObjectId) {
        self.project_mut()
            .unwrap()
            .targets
            .insert(object_id.clone());
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

    pub fn file_reference_id_by_path(&self, path_name: &str) -> Option<&ObjectId> {
        for object in &self.objects {
            if let (id, Object::FileReference(file_reference)) = object {
                if file_reference.path == path_name {
                    return Some(id);
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

    pub fn native_target_by_name_mut(&mut self, name: &str) -> Option<&mut NativeTarget> {
        for object in self.objects.values_mut() {
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

    pub fn configuration_by_id_mut(
        &mut self,
        object_id: &ObjectId,
    ) -> Option<&mut BuildConfiguration> {
        for object in self.objects.borrow_mut() {
            if let (id, Object::BuildConfiguration(configuration_list)) = object {
                if id == object_id {
                    return Some(configuration_list);
                }
            }
        }
        None
    }

    pub fn build_phase_by_id_mut(
        &mut self,
        object_id: &ObjectId,
    ) -> Option<&mut PbxCopyFilesBuildPhase> {
        for object in self.objects.borrow_mut() {
            if let (id, Object::CopyFilesBuildPhase(configuration_list)) = object {
                if id == object_id {
                    return Some(configuration_list);
                }
            }
        }
        None
    }

    pub fn create_plist_file(&mut self, relative_plist_path: &PathBuf) -> ObjectId {
        let object = ObjectId::new_random();

        self.objects.insert(
            object.clone(),
            Object::FileReference(PBXFileReference {
                last_known_file_type: Some("text.plist.xml".to_string()),
                name: Some(
                    relative_plist_path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                ),
                path: relative_plist_path.to_str().unwrap().to_string(),
                source_tree: "<group>".to_string(),

                file_encoding: None,
                include_in_index: None,
                explicit_file_type: None,
            }),
        );

        return object;
    }

    // pub fn create_target_dependency(&mut self, dependency_id: &ObjectId, proxy_id: &ObjectId) -> ObjectId {
    //     let id = ObjectId::new_random();

    //     self.objects.insert(id.clone(), Object::TargetDependency(PBXTargetDependency {
    //         target: dependency_id.clone(),
    //         target_proxy: proxy_id.clone()
    //     }));

    //     return id
    // }

    pub fn add_path(&mut self, path: &PathBuf) {
        let path_names: Vec<String> = path
            .components()
            .map(|x| x.as_os_str().to_str().unwrap().to_string())
            .collect();

        let mut target = self.project().unwrap().main_group.clone();

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

            let id = ObjectId::new_random();

            let new_child = PbxGroup {
                children: Default::default(),
                name: None,
                path: Some(path_name.clone()),
                source_tree: "<group>".to_string(),
            };

            self.objects.insert(id.clone(), Object::Group(new_child));
            self.group_mut(&target).unwrap().children.insert(id.clone());

            target = id;
        }
    }

    pub fn add_ref_to_group(&mut self, object_id: &ObjectId, group: &PathBuf) {
        let group_names: Vec<String> = group
            .components()
            .map(|x| x.as_os_str().to_str().unwrap().to_string())
            .collect();

        let mut object = self.project().unwrap().main_group.clone();

        for group_name in group_names {
            let children_references = &self.group(&object).unwrap().children;

            for child_reference in children_references {
                if let Some(child_group) = self.group(child_reference) {
                    if let Some(child_path) = &child_group.path {
                        if child_path == &group_name {
                            object = child_reference.clone();
                            break;
                        }
                    } else if let Some(child_name) = &child_group.name {
                        if child_name == &group_name {
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
            .insert(object_id.clone());
    }

    pub fn duplicate_target(
        &mut self,
        source_name: &str,
        destination_name: &str,
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
        new_native_target.name = destination_name.to_string();

        let new_configuration_list_id = ObjectId::new_random();
        let mut new_configuration_list = self
            .configuration_list_by_id(&new_native_target.build_configuration_list)
            .unwrap()
            .clone();

        // Create new build configurations
        let mut new_configuration_list_refs: BTreeSet<ObjectId> = BTreeSet::new();
        for build_configuration_id in &new_configuration_list.build_configurations {
            let new_build_configuration_id = ObjectId::new_random();
            let mut new_build_configuration = self
                .configuration_by_id(build_configuration_id)
                .unwrap()
                .clone();

            new_build_configuration.build_settings.insert(
                "INFOPLIST_FILE".to_string(),
                serde_json::Value::String(plist_path.to_str().unwrap().to_string()),
            );
            new_build_configuration.build_settings.insert(
                "PRODUCT_NAME".to_string(),
                serde_json::Value::String(destination_name.to_string()),
            );
            new_build_configuration.build_settings.insert(
                "CODE_SIGN_STYLE".to_string(),
                serde_json::Value::String("CODE_SIGN_STYLE".to_string()),
            );
            new_build_configuration.build_settings.insert(
                "PRODUCT_ENABLE_BITCODE".to_string(),
                serde_json::Value::String("ENABLE_BITCODE".to_string()),
            );

            new_configuration_list_refs.insert(new_build_configuration_id.clone());
            // Add to the actual .pbxproj
            // Each new build configuration
            self.objects.insert(
                new_build_configuration_id.clone(),
                Object::BuildConfiguration(new_build_configuration),
            );
        }
        // Add all new build configuration references to the list
        new_configuration_list.build_configurations =
            new_configuration_list_refs.into_iter().collect();

        // add new configuration list reference to the new native target
        new_native_target.build_configuration_list = new_configuration_list_id.clone();

        // Appex
        let new_appex_id = ObjectId::new_random();
        let mut new_appex = self
            .file_reference_by_id(&new_native_target.product_reference)
            .unwrap()
            .clone();
        new_appex.path = format!("{}.appex", destination_name);

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
        self.add_ref_to_group(&new_appex_id, &PathBuf::from_str("Products").unwrap());
        self.add_target(&new_appex_id);
    }

    pub fn set_target_build_configuration(&mut self, target_name: &str, key: &str, value: &str) {
        let target = self.native_target_by_name(&target_name).unwrap();
        let configuration_list = self
            .configuration_list_by_id(&target.build_configuration_list)
            .unwrap()
            .clone();

        for build_configuration_ref in &configuration_list.build_configurations {
            let new_build_configuration = self
                .configuration_by_id_mut(build_configuration_ref)
                .unwrap();
            new_build_configuration.build_settings.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    // TODO: Needs control flow cleanup
    pub fn add_appex_to_target_embedded_binaries(&mut self, target_path: &str, appex_path: &str) {
        let appex_id = self
            .file_reference_id_by_path(&format!("{}.appex", appex_path))
            .unwrap()
            .clone();

        let target = self.native_target_by_name(target_path).unwrap().clone();

        for build_phase_id in &target.build_phases {
            if let Some(build_phase) = self.build_phase_by_id_mut(build_phase_id) {
                if build_phase.name.as_ref().unwrap() == "Embed App Extensions" {
                    build_phase.files.insert(appex_id.clone());
                    break;
                }
            }
        }
        let build_file_id = ObjectId::new_random();
        self.objects.insert(
            build_file_id,
            Object::BuildFile(BuildFile {
                file_ref: appex_id,
                settings: None,
            }),
        );
    }

    pub fn remove_appex_from_target_embedded_binaries(
        &mut self,
        target_path: &str,
        appex_path: &str,
    ) {
        let appex_id = self
            .file_reference_id_by_path(&format!("{}.appex", appex_path))
            .unwrap()
            .clone();

        let target = self.native_target_by_name_mut(target_path).unwrap();

        // remove dependency that has been either automatically generated by xcode or us
        target.dependencies.remove(&appex_id);

        // remove build phase files
        for build_phase_id in target.build_phases.clone() {
            let build_phase = match self.build_phase_by_id_mut(&build_phase_id) {
                Some(x) => x,
                None => continue,
            };

            build_phase.files.remove(&appex_id);
        }
    }

    pub fn to_pbxproj_string(&self) -> String {
        let mut s = String::from("// !$*UTF8*$!\n{\n");

        s.push_str(&format!("\tarchiveVersion = {};\n", &self.archive_version));
        s.push_str("\tclasses = {\n");
        s.push_str("\t};\n");
        s.push_str(&format!("\tobjectVersion = {};\n", &self.object_version));

        // }};\n", &self.archive_version));
        s.push_str("\tobjects = {\n\n");

        s.push_str("/* Begin PBXBuildFile section */\n");
        for (oid, build_file) in iter_object!(self, BuildFile) {
            s.push_str(&format!("\t\t{} /* {} */ = {{", oid, "TODO"));
            s.push_str("isa = PBXBuildFile; fileRef = ");
            s.push_str(&format!("{} /* {} */; }};", build_file.file_ref, "TODO"));
            s.push('\n');
        }
        s.push_str("/* End PBXBuildFile section */\n\n");

        s.push_str("/* Begin PBXContainerItemProxy section */\n");
        for (oid, item_proxy) in iter_object!(self, ContainerItemProxy) {
            s.push_str(&format!("\t\t{} /* PBXContainerItemProxy */ = {{\n", oid));
            s.push_str("\t\t\tisa = PBXContainerItemProxy;\n");
            s.push_str(&format!(
                "\t\t\tcontainerPortal = {} /* {} */;\n",
                item_proxy.container_portal, "TODO"
            ));
            s.push_str(&format!("\t\t\tproxyType = {};\n", item_proxy.proxy_type));
            s.push_str(&format!(
                "\t\t\tremoteGlobalIDString = {};\n",
                item_proxy.remote_global_id_string
            ));
            s.push_str(&format!("\t\t\tremoteInfo = {};\n", item_proxy.remote_info));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXContainerItemProxy section */\n\n");

        s.push_str("/* Begin PBXCopyFilesBuildPhase section */\n");
        for (oid, phase) in iter_object!(self, CopyFilesBuildPhase) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = PBXCopyFilesBuildPhase;\n");
            s.push_str(&format!(
                "\t\t\tbuildActionMask = {};\n",
                phase.build_action_mask
            ));
            s.push_str(&format!("\t\t\tdstPath = {:?};\n", phase.dst_path));
            s.push_str(&format!(
                "\t\t\tdstSubfolderSpec = {};\n",
                phase.dst_subfolder_spec
            ));
            s.push_str("\t\t\tfiles = (\n");
            for file in phase.files.iter() {
                s.push_str(&format!("\t\t\t\t{} /* {} */;\n", file, "TODO"));
            }
            s.push_str("\t\t\t);\n");
            if let Some(name) = phase.name.as_ref() {
                s.push_str(&format!("\t\t\tname = {:?};\n", name));
            }
            s.push_str(&format!(
                "\t\t\trunOnlyForDeploymentPostprocessing = {};\n",
                phase.run_only_for_deployment_postprocessing
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXCopyFilesBuildPhase section */\n\n");

        s.push_str("/* Start PBXFileReference section */\n\n");

        for (oid, file_ref) in iter_object!(self, FileReference) {
            s.push_str(&format!(
                "\t\t{} /* {} */ = {{",
                oid,
                file_ref.name.as_deref().unwrap_or_else(|| &file_ref.path)
            ));
            s.push_str("isa = PBXFileReference; ");
            if let Some(x) = file_ref.file_encoding.as_ref() {
                s.push_str(&format!("fileEncoding = {}; ", x));
            }
            if let Some(x) = file_ref.include_in_index.as_ref() {
                s.push_str(&format!("includeInIndex = {}; ", x));
            }
            if let Some(x) = file_ref.last_known_file_type.as_ref() {
                s.push_str(&format!("lastKnownFileType = {}; ", x));
            }
            if let Some(x) = file_ref.name.as_ref() {
                s.push_str(&format!("name = {}; ", x));
            }
            s.push_str(&format!("path = {:?}; ", file_ref.path));
            s.push_str(&format!(
                "sourceTree = {}; ",
                if file_ref.source_tree == "<group>" {
                    "\"<group>\""
                } else {
                    &file_ref.source_tree
                }
            ));

            s.push_str("};\n");
        }
        s.push_str("/* End PBXFileReference section */\n\n");
        s.push_str("\t};\n}\n");
        s
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PBXCopyFilesBuildPhase {
    build_action_mask: String,
    dst_path: String,
    dst_subfolder_spec: String,
    files: BTreeSet<ObjectId>,
    name: Option<String>,
    run_only_for_deployment_postprocessing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "isa")]
pub enum Object {
    #[serde(rename = "PBXBuildFile")]
    BuildFile(BuildFile),

    #[serde(rename = "PBXFileReference")]
    FileReference(PBXFileReference),

    #[serde(rename = "PBXCopyFilesBuildPhase")]
    CopyFilesBuildPhase(PbxCopyFilesBuildPhase),

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
    TargetDependency(PBXTargetDependency),

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
    ContainerItemProxy(PBXContainerItemProxy),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PBXContainerItemProxy {
    pub container_portal: ObjectId,
    pub proxy_type: String,
    #[serde(rename = "remoteGlobalIDString")]
    pub remote_global_id_string: ObjectId,
    pub remote_info: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct BuildFile {
    pub settings: Option<IndexMap<String, serde_json::Value>>,
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
