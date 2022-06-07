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
pub struct PbxProject {
    attributes: serde_json::Value,
    build_configuration_list: ObjectId,
    compatibility_version: String,
    development_region: String,
    has_scanned_for_encodings: String,
    known_regions: BTreeSet<String>,
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

// TODO: currently same as PbxGroup, should we remove? Or will this vary?
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PbxVariantGroup {
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
    product_reference: Option<ObjectId>,
    product_name: String,
    build_phases: BTreeSet<ObjectId>,
    dependencies: BTreeSet<ObjectId>,
    name: String,
    build_rules: BTreeSet<String>,
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

// TODO: All 4 build phase types are almost identical, will they vary?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbxResourcesBuildPhase {
    #[serde(rename = "buildActionMask")]
    build_action_mask: String,
    files: BTreeSet<ObjectId>,
    name: Option<String>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    run_only_for_deployment_postprocessing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbxHeadersBuildPhase {
    #[serde(rename = "buildActionMask")]
    build_action_mask: String,
    files: BTreeSet<ObjectId>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    run_only_for_deployment_postprocessing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbxSourcesBuildPhase {
    #[serde(rename = "buildActionMask")]
    build_action_mask: String,
    files: BTreeSet<ObjectId>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    run_only_for_deployment_postprocessing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbxFrameworksBuildPhase {
    #[serde(rename = "buildActionMask")]
    build_action_mask: String,
    files: BTreeSet<ObjectId>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    run_only_for_deployment_postprocessing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbxShellScriptBuildPhase {
    #[serde(rename = "buildActionMask")]
    build_action_mask: String,
    files: BTreeSet<ObjectId>,
    #[serde(rename = "inputFileListPaths")]
    input_file_list_paths: Option<BTreeSet<String>>,
    #[serde(rename = "inputPaths")]
    input_paths: BTreeSet<String>,
    name: String,
    #[serde(rename = "outputFileListPaths")]
    output_file_list_paths: Option<BTreeSet<String>>,
    #[serde(rename = "outputPaths")]
    output_paths: BTreeSet<String>,
    #[serde(rename = "runOnlyForDeploymentPostprocessing")]
    run_only_for_deployment_postprocessing: String,
    #[serde(rename = "shellPath")]
    shell_path: String,
    #[serde(rename = "shellScript")]
    shell_script: String,
    #[serde(rename = "showEnvVarsInLog")]
    show_env_vars_in_log: Option<String>,
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

fn print_pbxproj_object_children(
    s: &mut String,
    key: Option<&str>,
    value: &serde_json::Value,
    indent: usize,
) {
    match value {
        serde_json::Value::Null => todo!(),
        serde_json::Value::Bool(_) => todo!(),
        serde_json::Value::Number(_) => todo!(),
        serde_json::Value::String(x) => {
            if let Some(k) = key {
                s.push_str(&format!("{x};\n"));
            } else {
                panic!("no associated key found for value");
            }
        }
        serde_json::Value::Array(_) => todo!(),
        serde_json::Value::Object(x) => {
            for (k, v) in x {
                for _i in 0..indent {
                    s.push_str("\t");
                }
                s.push_str(&format!("{k} = "));

                match v {
                    serde_json::Value::Object(_) => {
                        // for _i in 0..indent {
                        //     s.push_str("\t");
                        // }
                        s.push_str("{\n");
                        print_pbxproj_object_children(s, Some(k), v, indent + 1);
                        for _i in 0..indent {
                            s.push_str("\t");
                        }
                        s.push_str("};\n");
                    }
                    v => {
                        print_pbxproj_object_children(s, Some(k), v, indent + 1);
                    }
                }
            }
        }
    }
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

    pub fn known_regions_mut(&mut self) -> Option<&mut BTreeSet<String>> {
        if let Some(Object::Project(project)) = self.objects.get_mut(&self.root_object) {
            return Some(&mut project.known_regions);
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

    pub fn group_by_name_mut(&mut self, name: &str) -> Option<&mut PbxGroup> {
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

    pub fn variant_group_by_name_mut(&mut self, name: &str) -> Option<&mut PbxVariantGroup> {
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

    // TODO: rename with build phase name prefix
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

    // TODO: I hate this, but I'd love to move on, please future entity, fix my crimes
    pub fn resources_build_phase_by_target_name_mut(
        &mut self,
        target_name: &str,
    ) -> Option<&mut PbxResourcesBuildPhase> {
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

    // TODO: use create_file_reference instead, this has duplicate functionality
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

    pub fn create_file_reference(
        &mut self,
        file_type: &str,
        locale_name: &str,
        file_name: &str,
    ) -> ObjectId {
        let object_id = ObjectId::new_random();

        self.objects.insert(
            object_id.clone(),
            Object::FileReference(PBXFileReference {
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

        let variant_group = PbxVariantGroup {
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

        let variant_group = BuildFile {
            file_ref: file_ref.clone(),
            settings: None,
        };

        self.objects
            .insert(object_id.clone(), Object::BuildFile(variant_group));

        return object_id;
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

    // TODO: set product reference to null??
    pub fn remove_target(&mut self, target_name: &str) {
        let mut target = self.native_target_by_name_mut(target_name).unwrap();

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
                    let mut temp: BTreeMap<String, serde_json::Value> = BTreeMap::new();
                    temp.insert(
                        "ATTRIBUTES".to_string(),
                        serde_json::Value::Array(vec![serde_json::Value::String(
                            "RemoveHeadersOnCopy".to_string(),
                        )]),
                    );

                    let build_file_id = ObjectId::new_random();
                    self.objects.insert(
                        build_file_id,
                        Object::BuildFile(BuildFile {
                            file_ref: appex_id,
                            settings: Some(temp),
                        }),
                    );
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

        let target = self.native_target_by_name_mut(target_path).unwrap();

        // remove dependency that has been either automatically generated by xcode or us
        target.dependencies.remove(&appex_id);

        // remove build phase files
        for build_phase_id in target.build_phases.clone() {
            let build_phase = match self.build_phase_by_id_mut(&build_phase_id) {
                Some(x) => x,
                None => continue,
            };

            println!("REMOVE: {:?}", appex_id);

            build_phase.files.remove(&appex_id);
        }
    }

    pub fn update(&mut self, target_name: &str, locale_list: BTreeSet<String>) {
        // TODO: check that this is correct??
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

    // POSSIBLE ISSUES:
    // -Not clearing target product reference when removing target
    // -Not explicitly passing path and name when creating plist file
    // -Whole loop structure for layouts, locales, targets is wrong
    // -What is going on with project->knownRegions?
    // -File paths may have wrong root
    // -Line breaks in .pbxproj from build phases -> shellScript
    //
    // keywords: *todo* *fix* *println*

    fn list_to_pbxproj_string<T: AsRef<str>>(&self, item_iter: impl Iterator<Item = T>) -> String {
        let mut item_iter = item_iter.peekable();
        let mut item_string: String = String::new();
        while let Some(item) = item_iter.next() {
            item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_ref(), "TODO"));
            // if item_iter.peek().is_some() {
            //     item_string.push_str(",");
            // }
            item_string.push_str(",");
        }

        item_string
    }

    pub fn to_pbxproj_string(&self) -> String {
        let mut s = String::from("// !$*UTF8*$!\n{\n");

        s.push_str(&format!("\tarchiveVersion = {};\n", &self.archive_version));
        s.push_str("\tclasses = {\n");
        s.push_str("\t};\n");
        s.push_str(&format!("\tobjectVersion = {};\n", &self.object_version));

        // }};\n", &self.archive_version));
        s.push_str("\tobjects = {\n\n");

        // START PBXContainerItemProxy
        s.push_str("/* Begin PBXBuildFile section */\n");
        for (oid, build_file) in iter_object!(self, BuildFile) {
            s.push_str(&format!("\t\t{} /* {} */ = {{", oid, "TODO"));
            s.push_str("isa = PBXBuildFile; fileRef = ");
            s.push_str(&format!("{} /* {} */; ", build_file.file_ref, "TODO"));
            if let Some(settings) = build_file.settings.as_ref() {
                s.push_str("settings = {");
                for (key, values) in settings {
                    // println!("{:?}", values);
                    let mut settings_string: String = String::new();
                    for value_list in values.as_array() {
                        settings_string.push_str("(");
                        for value in value_list {
                            settings_string.push_str(&format!("{}, ", value.as_str().unwrap()));
                        }
                        settings_string.push_str(")");
                    }
                    s.push_str(&format!("{} = {};", key, settings_string));
                }
                s.push_str("};");
            }
            s.push_str(&format!("}};"));
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
        // END PBXContainerItemProxy

        // START PBXCopyFilesBuildPhase
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
                s.push_str(&format!("\t\t\t\t{} /* {} */,\n", file, "TODO"));
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
        // END PBXCopyFilesBuildPhase

        // START PBXFileReference
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
                // if x.contains('.') {
                //     s.push_str(&format!("name = {:?}; ", format!("{:?}", x)));
                // } else {
                //     s.push_str(&format!("name = {}; ", format!("{:?}", x)));
                // }
                if x.contains('-') || file_ref.path.contains('+') {
                    s.push_str(&format!("name = {:?}; ", x));
                } else {
                    s.push_str(&format!("name = {}; ", x));
                }
            }

            if file_ref.path.contains('-') || file_ref.path.contains('+') {
                s.push_str(&format!("path = {:?}; ", file_ref.path));
            } else {
                s.push_str(&format!("path = {}; ", file_ref.path));
            }

            s.push_str(&format!(
                "sourceTree = {}; ",
                if file_ref.source_tree == "<group>" {
                    "\"<group>\""
                } else {
                    &file_ref.source_tree
                }
            ));

            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXFileReference section */\n\n");
        // s.push_str("\t};\n}\n");
        // END PBXFileReference

        // START PBXFrameworksBuildPhase
        s.push_str("/* Start PBXFrameworksBuildPhase section */\n\n");
        for (oid, frameworks_build_phase) in iter_object!(self, FrameworksBuildPhase) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = PBXFrameworksBuildPhase;\n");
            s.push_str(&format!(
                "\t\t\tbuildActionMask = {};\n",
                frameworks_build_phase.build_action_mask
            ));
            {
                let mut item_string: String = String::new();
                let mut item_iter = frameworks_build_phase.files.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!("\t\t\tfiles = ({}\n\t\t\t);\n", item_string));
            }
            s.push_str(&format!(
                "\t\t\trunOnlyForDeploymentPostprocessing = {};\n",
                frameworks_build_phase.run_only_for_deployment_postprocessing
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXFrameworksBuildPhase section */\n\n");
        // s.push_str("\t};\n}\n");
        // END PBXFrameworksBuildPhase

        // START PBXGroup
        s.push_str("/* Start PBXGroup section */\n\n");
        for (oid, group) in iter_object!(self, Group) {
            if let Some(name) = group.name.as_deref() {
                s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, name));
            } else {
                if let Some(path) = group.path.as_deref() {
                    s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, path));
                } else {
                    s.push_str(&format!("\t\t{} = {{\n", oid));
                }
            }
            s.push_str("\t\t\tisa = PBXGroup;\n");

            let mut child_string: String = String::new();
            let mut child_iter = group.children.clone().into_iter().peekable();
            while let Some(child) = child_iter.next() {
                child_string.push_str(&format!("\n\t\t\t\t{} /* {} */", child.as_str(), "TODO"));
                child_string.push_str(",");
            }
            s.push_str(&format!("\t\t\tchildren = ({}\n\t\t\t);\n", child_string));

            if let Some(x) = group.path.as_ref() {
                if x.contains(' ') {
                    s.push_str(&format!("\t\t\tpath = {:?};\n", x));
                } else {
                    s.push_str(&format!("\t\t\tpath = {};\n", x));
                }
            }
            if let Some(x) = group.name.as_ref() {
                if x.contains(' ') {
                    s.push_str(&format!("\t\t\tname = {:?};\n", x));
                } else {
                    s.push_str(&format!("\t\t\tname = {};\n", x));
                }
            }
            s.push_str(&format!(
                "\t\t\tsourceTree = {};\n",
                if group.source_tree == "<group>" {
                    "\"<group>\""
                } else {
                    &group.source_tree
                }
            ));

            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXGroup section */\n\n");
        // END PBXGroup

        // START PBXHeadersBuildPhase
        s.push_str("/* Start PBXHeadersBuildPhase section */\n\n");
        for (oid, headers_build_phase) in iter_object!(self, HeadersBuildPhase) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = PBXHeadersBuildPhase;\n");
            s.push_str(&format!(
                "\t\t\tbuildActionMask = {};\n",
                headers_build_phase.build_action_mask
            ));
            {
                let mut item_string: String = String::new();
                let mut item_iter = headers_build_phase.files.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!("\t\t\tfiles = ({}\n\t\t\t);\n", item_string));
            }
            s.push_str(&format!(
                "\t\t\trunOnlyForDeploymentPostprocessing = {};\n",
                headers_build_phase.run_only_for_deployment_postprocessing
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXHeadersBuildPhase section */\n\n");
        // s.push_str("\t};\n}\n");
        // END PBXHeadersBuildPhase

        // START PBXNativeTarget
        s.push_str("/* Start PBXNativeTarget section */\n\n");
        for (oid, native_target) in iter_object!(self, NativeTarget) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = PBXNativeTarget;\n");
            s.push_str(&format!(
                "\t\t\tbuildConfigurationList = {};\n",
                native_target.build_configuration_list
            ));
            {
                let mut item_string: String = String::new();
                let mut item_iter = native_target.build_phases.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!("\t\t\tbuildPhases = ({}\n\t\t\t);\n", item_string));
            }
            {
                let mut item_string: String = String::new();
                let mut item_iter = native_target.build_rules.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!("\t\t\tbuildRules = ({}\n\t\t\t);\n", item_string));
            }
            {
                let mut item_string: String = String::new();
                let mut item_iter = native_target.dependencies.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!(
                    "\t\t\tdependencies = ({}\n\t\t\t);\n",
                    item_string
                ));
            }
            s.push_str(&format!("\t\t\tname = {:?};\n", native_target.name));
            s.push_str(&format!(
                "\t\t\tproductName = {};\n",
                native_target.product_name
            ));
            if let Some(product_reference) = native_target.product_reference.as_ref() {
                s.push_str(&format!(
                    "\t\t\tproductReference = {} /* {} */;\n",
                    product_reference, "TODO"
                ));
            }
            if let Some(product_type) = native_target.product_type.as_ref() {
                s.push_str(&format!("\t\t\tproductType = {};\n", product_type));
            }
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXNativeTarget section */\n\n");
        // s.push_str("\t};\n}\n");
        // END PBXNativeTarget

        // START PBXProject
        s.push_str("/* Start PBXProject section */\n\n");
        for (oid, project) in iter_object!(self, Project) {
            s.push_str(&format!("\t\t{} /* Project object */ = {{\n", oid));
            s.push_str("\t\t\tisa = PBXProject;\n");

            s.push_str("\t\t\tattributes = {\n");
            print_pbxproj_object_children(&mut s, None, &project.attributes, 4);
            s.push_str("\t\t\t};\n");

            s.push_str(&format!("\t\t\tbuildConfigurationList = {} /* TODO: Build configuration list for PBXProject \"GiellaKeyboard\" */;\n", project.build_configuration_list));
            s.push_str(&format!(
                "\t\t\tcompatibilityVersion = {:?};\n",
                project.compatibility_version
            ));
            s.push_str(&format!(
                "\t\t\tdevelopmentRegion = {};\n",
                project.development_region
            ));
            s.push_str(&format!(
                "\t\t\thasScannedForEncodings = {};\n",
                project.has_scanned_for_encodings
            ));
            {
                let mut item_string: String = String::new();
                let mut item_iter = project.known_regions.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{}", item.as_str()));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!(
                    "\t\t\tknownRegions = ({}\n\t\t\t);\n",
                    item_string
                ));
            }
            s.push_str(&format!("\t\t\tmainGroup = {};\n", project.main_group));
            s.push_str(&format!(
                "\t\t\tproductRefGroup = {} /* Products */;\n",
                project.product_ref_group
            ));
            s.push_str(&format!(
                "\t\t\tprojectDirPath = {:?};\n",
                project.project_dir_path
            ));
            s.push_str(&format!(
                "\t\t\tprojectRoot = {:?};\n",
                project.project_root
            ));

            s.push_str(&format!(
                "\t\t\ttargets = ({}\n\t\t\t);\n",
                self.list_to_pbxproj_string(project.targets.iter())
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXProject section */\n\n");
        // s.push_str("\t};\n}\n");
        // END PBXProject

        // START PBXResourcesBuildPhase
        s.push_str("/* Start PBXResourcesBuildPhase section */\n\n");
        for (oid, resources_build_phase) in iter_object!(self, ResourcesBuildPhase) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = PBXResourcesBuildPhase;\n");
            s.push_str(&format!(
                "\t\t\tbuildActionMask = {};\n",
                resources_build_phase.build_action_mask
            ));
            {
                let mut item_string: String = String::new();
                let mut item_iter = resources_build_phase.files.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!("\t\t\tfiles = ({}\n\t\t\t);\n", item_string));
            }
            s.push_str(&format!(
                "\t\t\trunOnlyForDeploymentPostprocessing = {};\n",
                resources_build_phase.run_only_for_deployment_postprocessing
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXResourcesBuildPhase section */\n\n");
        // s.push_str("\t};\n}\n");
        // END PBXResourcesBuildPhase

        // START PBXShellScriptBuildPhase
        s.push_str("/* Start PBXShellScriptBuildPhase section */\n\n");
        for (oid, shell_script_build_phase) in iter_object!(self, ShellScriptBuildPhase) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = PBXShellScriptBuildPhase;\n");
            s.push_str(&format!(
                "\t\t\tbuildActionMask = {};\n",
                shell_script_build_phase.build_action_mask
            ));
            {
                let mut item_string: String = String::new();
                let mut item_iter = shell_script_build_phase
                    .files
                    .clone()
                    .into_iter()
                    .peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!("\t\t\tfiles = ({}\n\t\t\t);\n", item_string));
            }
            if let Some(input_list_paths) = shell_script_build_phase.input_file_list_paths.as_ref()
            {
                let mut item_string: String = String::new();
                let mut item_iter = input_list_paths.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!(
                    "\t\t\tinputFileListPaths = ({}\n\t\t\t);\n",
                    item_string
                ));
            }
            {
                let mut item_string: String = String::new();
                let mut item_iter = shell_script_build_phase
                    .input_paths
                    .clone()
                    .into_iter()
                    .peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t\"{}\"", item.as_str()));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!("\t\t\tinputPaths = ({}\n\t\t\t);\n", item_string));
            }
            s.push_str(&format!(
                "\t\t\tname = {:?};\n",
                shell_script_build_phase.name
            ));
            if let Some(output_list_paths) =
                shell_script_build_phase.output_file_list_paths.as_ref()
            {
                let mut item_string: String = String::new();
                let mut item_iter = output_list_paths.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!(
                    "\t\t\toutputFileListPaths = ({}\n\t\t\t);\n",
                    item_string
                ));
            }
            {
                let mut item_string: String = String::new();
                let mut item_iter = shell_script_build_phase
                    .output_paths
                    .clone()
                    .into_iter()
                    .peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t\"{}\"", item.as_str()));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!("\t\t\toutputPaths = ({}\n\t\t\t);\n", item_string));
            }
            s.push_str(&format!(
                "\t\t\trunOnlyForDeploymentPostprocessing = {};\n",
                shell_script_build_phase.run_only_for_deployment_postprocessing
            ));
            s.push_str(&format!(
                "\t\t\tshellPath = {};\n",
                shell_script_build_phase.shell_path
            ));
            s.push_str(&format!(
                "\t\t\tshellScript = {:?};\n",
                shell_script_build_phase.shell_script
            ));
            if let Some(show_env_vars_in_log) =
                shell_script_build_phase.show_env_vars_in_log.as_ref()
            {
                s.push_str(&format!(
                    "\t\t\tshowEnvVarsInLog = {};\n",
                    show_env_vars_in_log
                ));
            }
            s.push_str("\t\t};\n")
        }
        s.push_str("/* End PBXShellScriptBuildPhase section */\n\n");
        // s.push_str("\t};\n}\n");
        // END PBXShellScriptBuildPhase

        // START PBXSourcesBuildPhase
        s.push_str("/* Start PBXSourcesBuildPhase section */\n\n");
        for (oid, sources_build_phase) in iter_object!(self, SourcesBuildPhase) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = PBXSourcesBuildPhase;\n");
            s.push_str(&format!(
                "\t\t\tbuildActionMask = {};\n",
                sources_build_phase.build_action_mask
            ));
            {
                let mut item_string: String = String::new();
                let mut item_iter = sources_build_phase.files.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!("\t\t\tfiles = ({}\n\t\t\t);\n", item_string));
            }
            s.push_str(&format!(
                "\t\t\trunOnlyForDeploymentPostprocessing = {};\n",
                sources_build_phase.run_only_for_deployment_postprocessing
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXSourcesBuildPhase section */\n\n");
        // s.push_str("\t};\n}\n");
        // END PBXSourcesBuildPhase

        // START PBXTargetDependency
        s.push_str("/* Start PBXTargetDependency section */\n\n");
        for (oid, target_dependency) in iter_object!(self, TargetDependency) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = PBXTargetDependency;\n");
            s.push_str(&format!("\t\t\ttarget = {};\n", target_dependency.target));
            s.push_str(&format!(
                "\t\t\ttargetProxy = {};\n",
                target_dependency.target_proxy
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXTargetDependency section */\n\n");
        // s.push_str("\t};\n}\n");
        // END PBXTargetDependency

        // START PBXVariantGroup
        s.push_str("/* Start PBXVariantGroup section */\n\n");
        for (oid, variant_group) in iter_object!(self, VariantGroup) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = PBXVariantGroup;\n");
            {
                let mut item_string: String = String::new();
                let mut item_iter = variant_group.children.clone().into_iter().peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!("\t\t\tchildren = ({}\n\t\t\t);\n", item_string));
            }
            if let Some(name) = variant_group.name.as_ref() {
                s.push_str(&format!("\t\t\tname = {:?};\n", name));
            }
            if let Some(path) = variant_group.path.as_ref() {
                println!("PATH: {:?}", variant_group.path);
                s.push_str(&format!("\t\t\tpath = {};\n", path));
            }
            s.push_str(&format!(
                "\t\t\tsourceTree = {};\n",
                if variant_group.source_tree == "<group>" {
                    "\"<group>\""
                } else {
                    &variant_group.source_tree
                }
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXVariantGroup section */\n\n");
        // s.push_str("\t};\n}\n");
        // END PBXVariantGroup

        // START XCBuildConfiguration
        s.push_str("/* Start XCBuildConfiguration section */\n\n");
        for (oid, build_configuration) in iter_object!(self, BuildConfiguration) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = XCBuildConfiguration;\n");
            {
                let mut item_string: String = String::new();
                let mut item_iter = build_configuration
                    .build_settings
                    .clone()
                    .into_iter()
                    .peekable();
                while let Some(item) = item_iter.next() {
                    match item.1 {
                        serde_json::Value::String(x) => {
                            let mut new_key = String::new();
                            let mut new_value = String::new();

                            if item.0.contains('[') {
                                new_key = format!("\"{}\"", item.0);
                            } else {
                                new_key = format!("{}", item.0);
                            }
                            new_value = format!("{:?}", x);

                            item_string
                                .push_str(&format!("\n\t\t\t\t{} = {};", new_key, new_value));
                        }
                        serde_json::Value::Array(a) => {
                            self.list_to_pbxproj_string(
                                a.iter().map(|x| x.as_str().unwrap()).peekable(),
                            );
                        }
                        _ => panic!("Disagreeable"),
                    }
                }
                s.push_str(&format!(
                    "\n\t\t\tbuildSettings = {{{}\n\t\t\t}};\n",
                    item_string
                ));
            }
            s.push_str(&format!("\t\t\tname = {:?};\n", build_configuration.name));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End XCBuildConfiguration section */\n\n");
        // END XCBuildConfiguration

        // START XCConfigurationList
        s.push_str("/* Start XCConfigurationList section */\n\n");
        for (oid, configuration_list) in iter_object!(self, ConfigurationList) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = XCConfigurationList;\n");
            {
                let mut item_string: String = String::new();
                let mut item_iter = configuration_list
                    .build_configurations
                    .clone()
                    .into_iter()
                    .peekable();
                while let Some(item) = item_iter.next() {
                    item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_str(), "TODO"));
                    // if item_iter.peek().is_some() {
                    //     item_string.push_str(",");
                    // }
                    item_string.push_str(",");
                }
                s.push_str(&format!(
                    "\t\t\tbuildConfigurations = ({}\n\t\t\t);\n",
                    item_string
                ));
            }
            s.push_str(&format!(
                "\t\t\tdefaultConfigurationIsVisible = {};\n",
                configuration_list.default_configuration_is_visible
            ));
            s.push_str(&format!(
                "\t\t\tdefaultConfigurationName = {};\n",
                configuration_list.default_configuration_name
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End XCConfigurationList section */\n\n");
        // s.push_str("\t};\n}\n");
        // END XCConfigurationList
        s.push_str("\t};\n");
        s.push_str(&format!("\trootObject = {};\n", self.root_object));
        s.push_str("}\n");
        return s;
    }
}

/*
    -Project

    --NativeTarget
    --FrameworksBuildPhase
    --SourcesBuildPhase
    --ShellScriptBuildPhase
    --BuildFile
    --FileReference
    --CopyFilesBuildPhase
    --Group
    --ConfigurationList
    --ResourcesBuildPhase
    --TargetDependency
    --VariantGroup
    --ContainerItemProxy
    --BuildConfiguration
    --HeadersBuildPhase
*/

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
    SourcesBuildPhase(PbxSourcesBuildPhase),

    #[serde(rename = "PBXFrameworksBuildPhase")]
    FrameworksBuildPhase(PbxFrameworksBuildPhase),

    #[serde(rename = "PBXResourcesBuildPhase")]
    ResourcesBuildPhase(PbxResourcesBuildPhase),

    #[serde(rename = "PBXTargetDependency")]
    TargetDependency(PBXTargetDependency),

    #[serde(rename = "PBXVariantGroup")]
    VariantGroup(PbxVariantGroup),

    #[serde(rename = "PBXShellScriptBuildPhase")]
    ShellScriptBuildPhase(PbxShellScriptBuildPhase),

    #[serde(rename = "PBXHeadersBuildPhase")]
    HeadersBuildPhase(PbxHeadersBuildPhase),

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
    pub settings: Option<BTreeMap<String, serde_json::Value>>,
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
