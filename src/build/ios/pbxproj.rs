use std::{
    borrow::BorrowMut,
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    str::FromStr,
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

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

impl AsRef<str> for ObjectId {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub attributes: serde_json::Value,
    pub build_configuration_list: ObjectId,
    pub compatibility_version: String,
    pub development_region: String,
    pub has_scanned_for_encodings: String,
    pub known_regions: BTreeSet<String>,
    pub main_group: ObjectId,
    pub product_ref_group: ObjectId,
    pub project_dir_path: String,
    pub project_root: String,
    pub targets: BTreeSet<ObjectId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct FileReference {
    pub file_encoding: Option<String>,
    pub include_in_index: Option<String>,
    pub last_known_file_type: Option<String>,
    pub explicit_file_type: Option<String>,
    pub name: Option<String>,
    pub path: String,
    pub source_tree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub children: BTreeSet<ObjectId>,
    pub source_tree: String,
    pub name: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct VariantGroup {
    pub children: BTreeSet<ObjectId>,
    pub source_tree: String,
    pub name: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct NativeTarget {
    pub product_type: Option<String>,
    pub build_configuration_list: ObjectId,
    pub product_reference: Option<ObjectId>,
    pub product_name: String,
    pub build_phases: BTreeSet<ObjectId>,
    pub dependencies: BTreeSet<ObjectId>,
    pub name: String,
    pub build_rules: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ConfigurationList {
    pub build_configurations: BTreeSet<ObjectId>,
    pub default_configuration_is_visible: String,
    pub default_configuration_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct BuildConfiguration {
    pub build_settings: IndexMap<String, serde_json::Value>,
    pub base_configuration_reference: Option<ObjectId>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct TargetDependency {
    pub target: ObjectId,
    pub target_proxy: ObjectId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyFilesBuildPhase {
    #[serde(rename = "buildActionMask")]
    pub build_action_mask: String,
    #[serde(rename = "dstPath")]
    pub dst_path: String,
    #[serde(rename = "dstSubfolderSpec")]
    pub dst_subfolder_spec: String,
    pub files: BTreeSet<ObjectId>,
    pub name: Option<String>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    pub run_only_for_deployment_postprocessing: String,
}

// All 4 build phase types are almost identical
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesBuildPhase {
    #[serde(rename = "buildActionMask")]
    pub build_action_mask: String,
    pub files: BTreeSet<ObjectId>,
    pub name: Option<String>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    pub run_only_for_deployment_postprocessing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadersBuildPhase {
    #[serde(rename = "buildActionMask")]
    pub build_action_mask: String,
    pub files: BTreeSet<ObjectId>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    pub run_only_for_deployment_postprocessing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcesBuildPhase {
    #[serde(rename = "buildActionMask")]
    pub build_action_mask: String,
    pub files: BTreeSet<ObjectId>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    pub run_only_for_deployment_postprocessing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworksBuildPhase {
    #[serde(rename = "buildActionMask")]
    pub build_action_mask: String,
    pub files: BTreeSet<ObjectId>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    pub run_only_for_deployment_postprocessing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellScriptBuildPhase {
    #[serde(rename = "buildActionMask")]
    pub build_action_mask: String,
    pub files: BTreeSet<ObjectId>,
    #[serde(rename = "inputFileListPaths")]
    pub input_file_list_paths: Option<BTreeSet<String>>,
    #[serde(rename = "inputPaths")]
    pub input_paths: BTreeSet<String>,
    pub name: String,
    #[serde(rename = "outputFileListPaths")]
    pub output_file_list_paths: Option<BTreeSet<String>>,
    #[serde(rename = "outputPaths")]
    pub output_paths: BTreeSet<String>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    pub run_only_for_deployment_postprocessing: String,
    #[serde(rename = "shellPath")]
    pub shell_path: String,
    #[serde(rename = "shellScript")]
    pub shell_script: String,
    #[serde(rename = "showEnvVarsInLog")]
    pub show_env_vars_in_log: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ContainerItemProxy {
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
    pub settings: Option<BTreeMap<String, serde_json::Value>>,
    pub file_ref: ObjectId,
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

impl Pbxproj {
    pub fn from_path(path: &Path) -> Self {
        convert_pbxproj_to_json(path)
    }

    pub fn project(&self) -> Option<&Project> {
        if let Some(Object::Project(project)) = self.objects.get(&self.root_object) {
            return Some(project);
        }
        None
    }

    pub fn project_mut(&mut self) -> Option<&mut Project> {
        if let Some(Object::Project(project)) = self.objects.get_mut(&self.root_object) {
            return Some(project);
        }
        None
    }

    pub fn known_regions_mut(&mut self) -> Option<&mut BTreeSet<String>> {
        if let Some(Object::Project(project)) = self.objects.get_mut(&self.root_object) {
            return Some(&mut project.known_regions);
        }
        None
    }

    pub fn add_target(&mut self, object_id: &ObjectId) {
        tracing::debug!("Adding target with ref: {}", &object_id);
        self.project_mut()
            .unwrap()
            .targets
            .insert(object_id.clone());
    }

    pub fn build_file_id_by_file_ref_name(&self, name: &str) -> Option<&ObjectId> {
        for object in &self.objects {
            if let (id, Object::BuildFile(build_file)) = object {
                if let Some(Object::FileReference(file_ref)) =
                    self.objects.get(&build_file.file_ref)
                {
                    if let Some(file_ref_name) = file_ref.name.as_ref() {
                        if file_ref_name == name {
                            return Some(id);
                        }
                    } else if file_ref.path == name {
                        return Some(id);
                    }
                }
            }
        }
        return None;
    }

    pub fn group(&self, object_id: &ObjectId) -> Option<&Group> {
        if let Some(Object::Group(main_group)) = self.objects.get(object_id) {
            return Some(main_group);
        }
        return None;
    }

    pub fn group_mut(&mut self, object_id: &ObjectId) -> Option<&mut Group> {
        if let Some(Object::Group(main_group)) = self.objects.get_mut(object_id) {
            return Some(main_group);
        }
        return None;
    }

    pub fn group_by_name_mut(&mut self, name: &str) -> Option<&mut Group> {
        for object in self.objects.borrow_mut() {
            if let (_id, Object::Group(group)) = object {
                if let Some(group_name) = group.name.clone() {
                    if group_name == name {
                        return Some(group);
                    }
                }
            }
        }
        None
    }

    pub fn variant_group_by_name_mut(&mut self, name: &str) -> Option<&mut VariantGroup> {
        for object in self.objects.borrow_mut() {
            if let (_id, Object::VariantGroup(group)) = object {
                if let Some(group_name) = group.name.clone() {
                    if group_name == name {
                        return Some(group);
                    }
                }
            }
        }
        None
    }

    pub fn file_reference_by_id(&self, object_id: &ObjectId) -> Option<&FileReference> {
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

    pub fn native_target_id_by_name(&mut self, name: &str) -> Option<&ObjectId> {
        for (key, value) in &self.objects {
            if let Object::NativeTarget(native_target) = value {
                if native_target.name == name {
                    return Some(key);
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

    pub fn copy_file_build_phase_by_id_mut(
        &mut self,
        object_id: &ObjectId,
    ) -> Option<&mut CopyFilesBuildPhase> {
        for object in self.objects.borrow_mut() {
            if let (id, Object::CopyFilesBuildPhase(configuration_list)) = object {
                if id == object_id {
                    return Some(configuration_list);
                }
            }
        }
        None
    }

    // TODO: Needs cleanup
    pub fn resources_build_phase_by_target_name_mut(
        &mut self,
        target_name: &str,
    ) -> Option<&mut ResourcesBuildPhase> {
        let mut the_id: Option<ObjectId> = None;

        if let Some(native_target) = self.native_target_by_name(target_name) {
            for build_phase_id in native_target.build_phases.clone() {
                if let Some(Object::ResourcesBuildPhase(_build_phase)) =
                    self.objects.get(&build_phase_id)
                {
                    the_id = Some(build_phase_id);
                }
            }
        }

        if let Some(phase_id) = the_id {
            for (object_id, object) in self.objects.borrow_mut() {
                if let Object::ResourcesBuildPhase(build_phase) = object {
                    if object_id == &phase_id {
                        return Some(build_phase);
                    }
                }
            }
        }

        None
    }

    // Very similar to create_file_reference
    pub fn create_plist_file(&mut self, relative_plist_path: &Path) -> ObjectId {
        let object = ObjectId::new_random();

        self.objects.insert(
            object.clone(),
            Object::FileReference(FileReference {
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

    pub fn create_file_reference(
        &mut self,
        file_type: &str,
        locale_name: &str,
        file_name: &str,
    ) -> ObjectId {
        let object_id = ObjectId::new_random();

        tracing::debug!(
            "Adding filereference {} for {} with ref: {}",
            &file_name,
            &locale_name,
            &object_id
        );

        self.objects.insert(
            object_id.clone(),
            Object::FileReference(FileReference {
                last_known_file_type: Some(file_type.to_string()),
                name: Some(locale_name.to_string()),
                path: format!("{}.lproj/{}", locale_name, file_name),
                source_tree: "<group>".to_string(),
                file_encoding: None,
                include_in_index: None,
                explicit_file_type: None,
            }),
        );

        return object_id;
    }

    pub fn create_variant_group(
        &mut self,
        children: BTreeSet<ObjectId>,
        name: Option<String>,
        path: Option<String>,
    ) -> ObjectId {
        let object_id = ObjectId::new_random();

        tracing::debug!("Adding variant group with ref: {}", &object_id);

        let variant_group = VariantGroup {
            children: children,
            source_tree: "<group>".to_string(),
            name: name,
            path: path,
        };

        self.objects
            .insert(object_id.clone(), Object::VariantGroup(variant_group));

        return object_id;
    }

    pub fn create_build_file(&mut self, file_ref: &ObjectId) -> ObjectId {
        let object_id = ObjectId::new_random();

        tracing::debug!("Adding build file with ref: {}", &object_id);

        let variant_group = BuildFile {
            file_ref: file_ref.clone(),
            settings: None,
        };

        self.objects
            .insert(object_id.clone(), Object::BuildFile(variant_group));

        return object_id;
    }

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

            let new_child = Group {
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

    pub fn add_file_ref_to_variant_group(&mut self, object_id: ObjectId) {
        let variant = self.variant_group_by_name_mut("About.txt").unwrap();
        variant.children.insert(object_id);
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
            // new_build_configuration.build_settings.insert(
            //     "CODE_SIGN_STYLE".to_string(),
            //     serde_json::Value::String("CODE_SIGN_STYLE".to_string()),
            // );
            // new_build_configuration.build_settings.insert(
            //     "PRODUCT_ENABLE_BITCODE".to_string(),
            //     serde_json::Value::String("ENABLE_BITCODE".to_string()),
            // );

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
            .file_reference_by_id(&new_native_target.product_reference.unwrap())
            .unwrap()
            .clone();
        new_appex.path = format!("{}.appex", destination_name);

        // Add new appex id to the new native target
        new_native_target.product_reference = Some(new_appex_id.clone());

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
            new_native_target_id.clone(),
            Object::NativeTarget(new_native_target),
        );

        // Finishing up
        self.add_ref_to_group(&new_appex_id, &PathBuf::from_str("Products").unwrap());
        self.add_target(&new_native_target_id);
    }

    pub fn remove_target(&mut self, target_name: &str) {
        let target = self.native_target_by_name_mut(target_name).unwrap();

        let product_reference = &target.product_reference.clone();
        target.product_reference = None;

        let target_id = self.native_target_id_by_name(target_name).unwrap().clone();

        let mut references_to_remove: BTreeSet<ObjectId> = BTreeSet::new();
        for (target_dependency_id, target_dependency) in &self.objects {
            if let Object::TargetDependency(target_dependency) = target_dependency {
                if target_dependency.target == target_id {
                    references_to_remove.insert(target_dependency_id.clone());
                }
            }
        }

        for reference in references_to_remove {
            self.objects.remove(&reference);
            for obj in self.objects.borrow_mut() {
                if let (_any_target_id, Object::NativeTarget(any_target)) = obj {
                    any_target.dependencies.remove(&reference);
                }
            }
        }

        self.group_by_name_mut("Products")
            .unwrap()
            .children
            .remove(product_reference.as_ref().unwrap());

        self.project_mut().unwrap().targets.remove(&target_id);
        self.objects.remove(&target_id);
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

    pub fn add_appex_to_target_embedded_binaries(&mut self, target_path: &str, appex_path: &str) {
        let appex_id = self
            .file_reference_id_by_path(&format!("{}.appex", appex_path))
            .unwrap()
            .clone();

        let target = self.native_target_by_name(target_path).unwrap().clone();

        for build_phase_id in &target.build_phases {
            if let Some(build_phase) = self.copy_file_build_phase_by_id_mut(build_phase_id) {
                if build_phase.name.as_ref().unwrap() == "Embed App Extensions" || build_phase.name.as_ref().unwrap() == "Embed Foundation Extensions" {
                    let build_file_id = ObjectId::new_random();
                    // Insert new build file that references our new keyboard appex
                    build_phase.files.insert(build_file_id.clone());

                    // Create build file that references our keyboard appex
                    let mut temp: BTreeMap<String, serde_json::Value> = BTreeMap::new();
                    temp.insert(
                        "ATTRIBUTES".to_string(),
                        serde_json::Value::Array(vec![serde_json::Value::String(
                            "RemoveHeadersOnCopy".to_string(),
                        )]),
                    );
                    {
                        self.objects.insert(
                            build_file_id,
                            Object::BuildFile(BuildFile {
                                file_ref: appex_id,
                                settings: Some(temp),
                            }),
                        );
                    }
                    break;
                }
            }
        }
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

        let build_file_id_to_remove = self
            .build_file_id_by_file_ref_name(&format!("{}.appex", appex_path))
            .unwrap()
            .clone();

        let target = self.native_target_by_name_mut(target_path).unwrap();

        // remove dependency that has been either automatically generated by xcode or us
        target.dependencies.remove(&appex_id);

        // remove build phase files
        for build_phase_id in target.build_phases.clone() {
            match self.copy_file_build_phase_by_id_mut(&build_phase_id) {
                Some(x) => {
                    if x.name.as_ref().unwrap() == "Embed App Extensions" || x.name.as_ref().unwrap() == "Embed Foundation Extensions" {
                        x.files.remove(&build_file_id_to_remove);
                        return;
                    }
                }
                None => continue,
            };
        }
    }

    pub fn update(&mut self, target_name: &str, locale_list: BTreeSet<String>) {
        tracing::debug!("Updating target {} with new locales", &target_name);

        let known_regions = self.known_regions_mut().unwrap();
        known_regions.extend(locale_list.clone());

        let mut new_locale_ids: BTreeSet<ObjectId> = BTreeSet::new();
        for locale in locale_list {
            // create and add plist file: self.create_file_reference("text.plist.strings", locale, name)
            let temp =
                self.create_file_reference("text.plist.strings", &locale, "InfoPlist.strings");
            new_locale_ids.insert(temp);
        }

        // create and add variant group for locales
        let variant_group_id =
            self.create_variant_group(new_locale_ids, Some("InfoPlist.strings".to_string()), None);
        // create buildfile
        let locale_group_build_file_id = self.create_build_file(&variant_group_id);
        // add buildfile reference to resources phase files
        let target_resources_phase = self
            .resources_build_phase_by_target_name_mut(target_name)
            .unwrap();
        target_resources_phase
            .files
            .insert(locale_group_build_file_id);
        // add variant reference to HostingApp/SupportingFiles group
        self.add_ref_to_group(
            &variant_group_id,
            &PathBuf::from_str("HostingApp/Supporting Files").unwrap(),
        );
    }

    pub fn set_build_target_setting(&mut self, target_name: &str, key: &str, value: &str) {
        tracing::debug!(
            target_name = target_name,
            key = key,
            value = value,
            "Setting build target setting"
        );
        let target = self.native_target_by_name(target_name).unwrap().clone();
        let list = self
            .configuration_list_by_id(&target.build_configuration_list)
            .unwrap()
            .clone();

        for id in list.build_configurations.iter() {
            let cfg = self.configuration_by_id_mut(id).unwrap();
            cfg.build_settings.insert(key.to_string(), value.into());
        }
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
    FileReference(FileReference),

    #[serde(rename = "PBXCopyFilesBuildPhase")]
    CopyFilesBuildPhase(CopyFilesBuildPhase),

    #[serde(rename = "PBXGroup")]
    Group(Group),

    #[serde(rename = "PBXProject")]
    Project(Project),

    #[serde(rename = "XCConfigurationList")]
    ConfigurationList(ConfigurationList),

    #[serde(rename = "PBXSourcesBuildPhase")]
    SourcesBuildPhase(SourcesBuildPhase),

    #[serde(rename = "PBXFrameworksBuildPhase")]
    FrameworksBuildPhase(FrameworksBuildPhase),

    #[serde(rename = "PBXResourcesBuildPhase")]
    ResourcesBuildPhase(ResourcesBuildPhase),

    #[serde(rename = "PBXTargetDependency")]
    TargetDependency(TargetDependency),

    #[serde(rename = "PBXVariantGroup")]
    VariantGroup(VariantGroup),

    #[serde(rename = "PBXShellScriptBuildPhase")]
    ShellScriptBuildPhase(ShellScriptBuildPhase),

    #[serde(rename = "PBXHeadersBuildPhase")]
    HeadersBuildPhase(HeadersBuildPhase),

    #[serde(rename = "PBXNativeTarget")]
    NativeTarget(NativeTarget),

    #[serde(rename = "XCBuildConfiguration")]
    BuildConfiguration(BuildConfiguration),

    #[serde(rename = "PBXContainerItemProxy")]
    ContainerItemProxy(ContainerItemProxy),
}

pub fn convert_pbxproj_to_json(path: &Path) -> Pbxproj {
    tracing::debug!("Getting .pbxproj as json");

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
