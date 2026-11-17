use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::Command,
};

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::ensure_success;

#[nova::newtype(serde, display)]
pub type ObjectId = String;

impl ObjectId {
    // [spec:kbdgen:req:pbxproj.ids]
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
pub struct Project {
    pub attributes: serde_json::Map<String, serde_json::Value>,
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
    pub package_references: Option<BTreeSet<ObjectId>>,
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

// [spec:kbdgen:sem:pbxproj.model.ordering+1]
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
    pub package_product_dependencies: Option<BTreeSet<ObjectId>>,
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
    pub file_ref: Option<ObjectId>,
    /// A Swift Package Manager product. A build file has this or `file_ref`.
    pub product_ref: Option<ObjectId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RemoteSwiftPackageReference {
    #[serde(rename = "repositoryURL")]
    pub repository_url: String,
    pub requirement: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct SwiftPackageProductDependency {
    pub package: Option<ObjectId>,
    pub product_name: String,
}

// [spec:kbdgen:def:pbxproj.model+2]
// [spec:kbdgen:sem:pbxproj.model.ordering+1]
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

/// Newer Xcode versions name the phase that embeds app extensions "Embed Foundation Extensions".
const EMBED_EXTENSIONS_PHASES: [&str; 2] = ["Embed App Extensions", "Embed Foundation Extensions"];

impl Pbxproj {
    /// Reads a project.pbxproj by having `plutil`, which only ships with macOS, convert a copy of
    /// it to JSON.
    // [spec:kbdgen:req:pbxproj.model.read+1]
    pub fn from_path(path: &Path) -> Result<Self> {
        let tempdir = tempfile::tempdir().context("create a temporary directory")?;
        let json_path = tempdir.path().join("tmp.pbxproj");
        std::fs::copy(path, &json_path)
            .with_context(|| format!("copy {} to {}", path.display(), json_path.display()))?;

        let output = Command::new("plutil")
            .args(["-convert", "json"])
            .arg(&json_path)
            .output()
            .context("`plutil` could not be started; it ships with macOS")?;
        ensure_success(&format!("plutil -convert json {}", path.display()), &output)?;

        let json =
            std::fs::read(&json_path).with_context(|| format!("read {}", json_path.display()))?;
        serde_json::from_slice(&json).with_context(|| format!("parse {}", path.display()))
    }

    fn project(&self) -> Result<&Project> {
        match self.objects.get(&self.root_object) {
            Some(Object::Project(project)) => Ok(project),
            _ => bail!("rootObject {} is not a PBXProject", self.root_object),
        }
    }

    fn project_mut(&mut self) -> Result<&mut Project> {
        match self.objects.get_mut(&self.root_object) {
            Some(Object::Project(project)) => Ok(project),
            _ => bail!("rootObject {} is not a PBXProject", self.root_object),
        }
    }

    fn group(&self, id: &ObjectId) -> Option<&Group> {
        match self.objects.get(id) {
            Some(Object::Group(group)) => Some(group),
            _ => None,
        }
    }

    fn group_mut(&mut self, id: &ObjectId) -> Result<&mut Group> {
        match self.objects.get_mut(id) {
            Some(Object::Group(group)) => Ok(group),
            _ => bail!("{id} is not a PBXGroup"),
        }
    }

    fn file_reference(&self, id: &ObjectId) -> Result<&FileReference> {
        match self.objects.get(id) {
            Some(Object::FileReference(file_reference)) => Ok(file_reference),
            _ => bail!("{id} is not a PBXFileReference"),
        }
    }

    fn configuration_list(&self, id: &ObjectId) -> Result<&ConfigurationList> {
        match self.objects.get(id) {
            Some(Object::ConfigurationList(list)) => Ok(list),
            _ => bail!("{id} is not an XCConfigurationList"),
        }
    }

    fn build_configuration(&self, id: &ObjectId) -> Result<&BuildConfiguration> {
        match self.objects.get(id) {
            Some(Object::BuildConfiguration(configuration)) => Ok(configuration),
            _ => bail!("{id} is not an XCBuildConfiguration"),
        }
    }

    fn build_configuration_mut(&mut self, id: &ObjectId) -> Result<&mut BuildConfiguration> {
        match self.objects.get_mut(id) {
            Some(Object::BuildConfiguration(configuration)) => Ok(configuration),
            _ => bail!("{id} is not an XCBuildConfiguration"),
        }
    }

    fn native_target(&self, name: &str) -> Result<(&ObjectId, &NativeTarget)> {
        self.objects
            .iter()
            .find_map(|(id, object)| match object {
                Object::NativeTarget(target) if target.name == name => Some((id, target)),
                _ => None,
            })
            .with_context(|| format!("the project has no target named {name}"))
    }

    fn build_file_id_by_file_ref_name(&self, name: &str) -> Option<&ObjectId> {
        self.objects.iter().find_map(|(id, object)| {
            let Object::BuildFile(build_file) = object else {
                return None;
            };
            let Some(Object::FileReference(file_ref)) =
                self.objects.get(build_file.file_ref.as_ref()?)
            else {
                return None;
            };
            (file_ref.name.as_deref().unwrap_or(&file_ref.path) == name).then_some(id)
        })
    }

    fn file_reference_id_by_path(&self, path: &str) -> Option<&ObjectId> {
        self.objects.iter().find_map(|(id, object)| match object {
            Object::FileReference(file_reference) if file_reference.path == path => Some(id),
            _ => None,
        })
    }

    fn embed_extensions_phase_id(&self, target_name: &str) -> Result<ObjectId> {
        let (_, target) = self.native_target(target_name)?;
        target
            .build_phases
            .iter()
            .find(|id| match self.objects.get(id) {
                Some(Object::CopyFilesBuildPhase(phase)) => phase
                    .name
                    .as_deref()
                    .is_some_and(|name| EMBED_EXTENSIONS_PHASES.contains(&name)),
                _ => false,
            })
            .cloned()
            .with_context(|| format!("target {target_name} has no Embed App Extensions phase"))
    }

    fn create_variant_group(&mut self, children: BTreeSet<ObjectId>, name: &str) -> ObjectId {
        let id = ObjectId::new_random();
        let group = VariantGroup {
            children,
            source_tree: "<group>".to_string(),
            name: Some(name.to_string()),
            path: None,
        };
        self.objects.insert(id.clone(), Object::VariantGroup(group));
        id
    }

    fn create_build_file(&mut self, file_ref: &ObjectId) -> ObjectId {
        let id = ObjectId::new_random();
        let build_file = BuildFile {
            file_ref: Some(file_ref.clone()),
            product_ref: None,
            settings: None,
        };
        self.objects
            .insert(id.clone(), Object::BuildFile(build_file));
        id
    }

    /// Creates a reference to `<locale>.lproj/<file_name>`, named for its locale as the children
    /// of a variant group are.
    fn create_localized_file_reference(
        &mut self,
        file_type: &str,
        locale: &str,
        file_name: &str,
    ) -> ObjectId {
        let id = ObjectId::new_random();
        self.objects.insert(
            id.clone(),
            Object::FileReference(FileReference {
                last_known_file_type: Some(file_type.to_string()),
                name: Some(locale.to_string()),
                path: format!("{locale}.lproj/{file_name}"),
                source_tree: "<group>".to_string(),
                file_encoding: None,
                include_in_index: None,
                explicit_file_type: None,
            }),
        );
        id
    }

    pub fn create_plist_file_reference(&mut self, path: &Path) -> ObjectId {
        let id = ObjectId::new_random();
        self.objects.insert(
            id.clone(),
            Object::FileReference(FileReference {
                last_known_file_type: Some("text.plist.xml".to_string()),
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned()),
                path: path.to_string_lossy().into_owned(),
                source_tree: "<group>".to_string(),
                file_encoding: None,
                include_in_index: None,
                explicit_file_type: None,
            }),
        );
        id
    }

    /// Adds `<locale>.lproj/About.txt` to the `About.txt` variant group unless it is already there.
    pub fn add_localized_about(&mut self, locale: &str) -> Result<()> {
        let path = format!("{locale}.lproj/About.txt");
        let (group_id, group) = self
            .objects
            .iter()
            .find_map(|(id, object)| match object {
                Object::VariantGroup(group) if group.name.as_deref() == Some("About.txt") => {
                    Some((id.clone(), group))
                }
                _ => None,
            })
            .context("the project has no About.txt variant group")?;
        let present = group.children.iter().any(|child| {
            self.file_reference(child)
                .is_ok_and(|file| file.path == path)
        });
        if present {
            return Ok(());
        }

        let id = self.create_localized_file_reference("text", locale, "About.txt");
        let Some(Object::VariantGroup(group)) = self.objects.get_mut(&group_id) else {
            unreachable!("{group_id} was found as a variant group");
        };
        group.children.insert(id);
        Ok(())
    }

    /// Makes sure the main group has a nested group, matched by `path`, for each component of
    /// `path`.
    pub fn add_path(&mut self, path: &Path) -> Result<()> {
        let mut parent = self.project()?.main_group.clone();

        for component in path.components() {
            let component = component.as_os_str().to_string_lossy();
            let children = &self
                .group(&parent)
                .with_context(|| format!("{parent} is not a PBXGroup"))?
                .children;
            let existing = children.iter().find(|child| {
                self.group(child)
                    .is_some_and(|group| group.path.as_deref() == Some(&*component))
            });

            parent = match existing {
                Some(child) => child.clone(),
                None => {
                    let id = ObjectId::new_random();
                    let group = Group {
                        children: Default::default(),
                        name: None,
                        path: Some(component.into_owned()),
                        source_tree: "<group>".to_string(),
                    };
                    self.objects.insert(id.clone(), Object::Group(group));
                    self.group_mut(&parent)?.children.insert(id.clone());
                    id
                }
            };
        }

        Ok(())
    }

    /// Adds `id` to the group at `group_path`, matching each component against a child group's
    /// `path`, or its `name` if it has no path. A component that matches no group is skipped, so
    /// `id` lands in the deepest group found.
    pub fn add_ref_to_group(&mut self, id: &ObjectId, group_path: &Path) -> Result<()> {
        let mut group_id = self.project()?.main_group.clone();

        for component in group_path.components() {
            let component = component.as_os_str().to_string_lossy();
            let children = &self
                .group(&group_id)
                .with_context(|| format!("{group_id} is not a PBXGroup"))?
                .children;
            let child = children.iter().find(|child| {
                self.group(child).is_some_and(|group| {
                    group.path.as_deref().or(group.name.as_deref()) == Some(&*component)
                })
            });
            if let Some(child) = child {
                group_id = child.clone();
            }
        }

        self.group_mut(&group_id)?.children.insert(id.clone());
        Ok(())
    }

    // [spec:kbdgen:req:xcode.targets+2]
    pub fn duplicate_target(
        &mut self,
        source_name: &str,
        destination_name: &str,
        plist_path: &Path,
    ) -> Result<()> {
        let mut target = self.native_target(source_name)?.1.clone();
        let target_id = ObjectId::new_random();
        target.name = destination_name.to_string();

        let configuration_list_id = ObjectId::new_random();
        let mut configuration_list = self
            .configuration_list(&target.build_configuration_list)?
            .clone();

        let mut configuration_ids = BTreeSet::new();
        for source_id in &configuration_list.build_configurations {
            let id = ObjectId::new_random();
            let mut configuration = self.build_configuration(source_id)?.clone();
            configuration.build_settings.insert(
                "INFOPLIST_FILE".to_string(),
                plist_path.to_string_lossy().into(),
            );
            configuration
                .build_settings
                .insert("PRODUCT_NAME".to_string(), destination_name.into());
            self.objects
                .insert(id.clone(), Object::BuildConfiguration(configuration));
            configuration_ids.insert(id);
        }
        configuration_list.build_configurations = configuration_ids;
        target.build_configuration_list = configuration_list_id.clone();

        let appex_id = ObjectId::new_random();
        let source_appex_id = target
            .product_reference
            .as_ref()
            .with_context(|| format!("target {source_name} has no product"))?;
        let mut appex = self.file_reference(source_appex_id)?.clone();
        appex.path = format!("{destination_name}.appex");
        target.product_reference = Some(appex_id.clone());

        self.objects
            .insert(appex_id.clone(), Object::FileReference(appex));
        self.objects.insert(
            configuration_list_id,
            Object::ConfigurationList(configuration_list),
        );
        self.objects
            .insert(target_id.clone(), Object::NativeTarget(target));

        self.add_ref_to_group(&appex_id, Path::new("Products"))?;
        self.project_mut()?.targets.insert(target_id);
        Ok(())
    }

    /// Removes the target, the dependencies on it and its product's entry in `Products`. Its
    /// product file reference stays: `remove_appex_from_target_embedded_binaries` finds the
    /// embedding build file through it.
    // [spec:kbdgen:req:xcode.targets+2]
    pub fn remove_target(&mut self, name: &str) -> Result<()> {
        let (target_id, target) = self.native_target(name)?;
        let target_id = target_id.clone();
        let product_id = target
            .product_reference
            .clone()
            .with_context(|| format!("target {name} has no product"))?;

        let dependency_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, object)| match object {
                Object::TargetDependency(dependency) if dependency.target == target_id => {
                    Some(id.clone())
                }
                _ => None,
            })
            .collect();
        for dependency_id in &dependency_ids {
            self.objects.remove(dependency_id);
        }
        for object in self.objects.values_mut() {
            if let Object::NativeTarget(target) = object {
                for dependency_id in &dependency_ids {
                    target.dependencies.remove(dependency_id);
                }
            }
        }

        let products = self
            .objects
            .values_mut()
            .find_map(|object| match object {
                Object::Group(group) if group.name.as_deref() == Some("Products") => Some(group),
                _ => None,
            })
            .context("the project has no Products group")?;
        products.children.remove(&product_id);

        self.project_mut()?.targets.remove(&target_id);
        self.objects.remove(&target_id);
        Ok(())
    }

    pub fn set_build_setting(&mut self, target_name: &str, key: &str, value: &str) -> Result<()> {
        tracing::debug!("Setting {key} = {value} for target {target_name}");
        let list_id = &self.native_target(target_name)?.1.build_configuration_list;
        let configuration_ids = self
            .configuration_list(list_id)?
            .build_configurations
            .clone();
        for id in &configuration_ids {
            self.build_configuration_mut(id)?
                .build_settings
                .insert(key.to_string(), value.into());
        }
        Ok(())
    }

    // [spec:kbdgen:req:xcode.targets+2]
    pub fn add_appex_to_target_embedded_binaries(
        &mut self,
        target_name: &str,
        appex_name: &str,
    ) -> Result<()> {
        let appex = format!("{appex_name}.appex");
        let appex_id = self
            .file_reference_id_by_path(&appex)
            .with_context(|| format!("the project has no file reference for {appex}"))?
            .clone();
        let phase_id = self.embed_extensions_phase_id(target_name)?;

        let build_file_id = ObjectId::new_random();
        let Some(Object::CopyFilesBuildPhase(phase)) = self.objects.get_mut(&phase_id) else {
            unreachable!("embed_extensions_phase_id returns a copy-files phase");
        };
        phase.files.insert(build_file_id.clone());

        let settings = BTreeMap::from([(
            "ATTRIBUTES".to_string(),
            serde_json::json!(["RemoveHeadersOnCopy"]),
        )]);
        self.objects.insert(
            build_file_id,
            Object::BuildFile(BuildFile {
                file_ref: Some(appex_id),
                product_ref: None,
                settings: Some(settings),
            }),
        );
        Ok(())
    }

    pub fn remove_appex_from_target_embedded_binaries(
        &mut self,
        target_name: &str,
        appex_name: &str,
    ) -> Result<()> {
        let appex = format!("{appex_name}.appex");
        let build_file_id = self
            .build_file_id_by_file_ref_name(&appex)
            .with_context(|| format!("the project has no build file for {appex}"))?
            .clone();
        let phase_id = self.embed_extensions_phase_id(target_name)?;

        let Some(Object::CopyFilesBuildPhase(phase)) = self.objects.get_mut(&phase_id) else {
            unreachable!("embed_extensions_phase_id returns a copy-files phase");
        };
        phase.files.remove(&build_file_id);
        Ok(())
    }

    /// Adds `locales` to the project's known regions, and their `InfoPlist.strings` to the
    /// target's resources and to the `HostingApp/Supporting Files` group.
    // [spec:kbdgen:req:xcode.localization+1]
    pub fn add_locales(&mut self, target_name: &str, locales: &BTreeSet<String>) -> Result<()> {
        self.project_mut()?
            .known_regions
            .extend(locales.iter().cloned());

        let strings_ids = locales
            .iter()
            .map(|locale| {
                self.create_localized_file_reference(
                    "text.plist.strings",
                    locale,
                    "InfoPlist.strings",
                )
            })
            .collect();
        let variant_group_id = self.create_variant_group(strings_ids, "InfoPlist.strings");
        let build_file_id = self.create_build_file(&variant_group_id);

        let (_, target) = self.native_target(target_name)?;
        let resources_phase_id = target
            .build_phases
            .iter()
            .rev()
            .find(|id| matches!(self.objects.get(id), Some(Object::ResourcesBuildPhase(_))))
            .cloned()
            .with_context(|| format!("target {target_name} has no resources build phase"))?;
        let Some(Object::ResourcesBuildPhase(resources_phase)) =
            self.objects.get_mut(&resources_phase_id)
        else {
            unreachable!("the phase was found as a resources build phase");
        };
        resources_phase.files.insert(build_file_id);

        self.add_ref_to_group(&variant_group_id, Path::new("HostingApp/Supporting Files"))
    }
}

// [spec:kbdgen:def:pbxproj.model+2]
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

    #[serde(rename = "XCRemoteSwiftPackageReference")]
    RemoteSwiftPackageReference(RemoteSwiftPackageReference),

    #[serde(rename = "XCSwiftPackageProductDependency")]
    SwiftPackageProductDependency(SwiftPackageProductDependency),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn about_project() -> Pbxproj {
        serde_json::from_value(serde_json::json!({
            "archiveVersion": "1",
            "classes": {},
            "objectVersion": "52",
            "rootObject": "P0",
            "objects": {
                "V0": {
                    "isa": "PBXVariantGroup",
                    "children": ["F0"],
                    "name": "About.txt",
                    "sourceTree": "<group>"
                },
                "F0": {
                    "isa": "PBXFileReference",
                    "lastKnownFileType": "text",
                    "name": "Base",
                    "path": "Base.lproj/About.txt",
                    "sourceTree": "<group>"
                }
            }
        }))
        .unwrap()
    }

    fn about_paths(pbxproj: &Pbxproj) -> Vec<&str> {
        let Some(Object::VariantGroup(group)) = pbxproj.objects.get(&ObjectId("V0".into())) else {
            panic!("V0 is not a variant group");
        };
        let mut paths: Vec<&str> = group
            .children
            .iter()
            .map(|id| pbxproj.file_reference(id).unwrap().path.as_str())
            .collect();
        paths.sort();
        paths
    }

    // [spec:kbdgen:req:xcode.localization+1/test]
    #[test]
    fn add_localized_about_adds_each_locale_once() {
        let mut pbxproj = about_project();

        pbxproj.add_localized_about("Base").unwrap();
        pbxproj.add_localized_about("nb").unwrap();
        pbxproj.add_localized_about("nb").unwrap();

        assert_eq!(
            about_paths(&pbxproj),
            ["Base.lproj/About.txt", "nb.lproj/About.txt"]
        );
    }

    // [spec:kbdgen:req:xcode.targets+2/test]
    #[test]
    fn missing_target_is_an_error() {
        let mut pbxproj = about_project();

        let error = pbxproj.remove_target("Keyboard").unwrap_err();

        assert_eq!(
            error.to_string(),
            "the project has no target named Keyboard"
        );
    }

    // [spec:kbdgen:def:pbxproj.model+2/test]
    #[test]
    fn project_attributes_must_be_a_dictionary() {
        let project = |attributes: serde_json::Value| {
            serde_json::from_value::<Object>(serde_json::json!({
                "isa": "PBXProject",
                "attributes": attributes,
                "buildConfigurationList": "CL",
                "compatibilityVersion": "Xcode 3.2",
                "developmentRegion": "en",
                "hasScannedForEncodings": "0",
                "knownRegions": [],
                "mainGroup": "G0",
                "productRefGroup": "G1",
                "projectDirPath": "",
                "projectRoot": "",
                "targets": []
            }))
        };

        assert!(project(serde_json::json!({ "LastUpgradeCheck": "1130" })).is_ok());
        assert!(project(serde_json::json!("1130")).is_err());
    }

    // [spec:kbdgen:req:pbxproj.model.read+1/test/golden]
    #[cfg(target_os = "macos")]
    #[test]
    fn reads_what_it_writes_through_plutil() {
        let golden =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/build/ios/testdata/every-kind.pbxproj");

        let written = Pbxproj::from_path(&golden).unwrap().to_pbxproj_string();

        // plutil does not keep the file's buildSettings key order, so only the lines are compared.
        let sorted_lines = |text: &str| {
            let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
            lines.sort();
            lines
        };
        assert_eq!(
            sorted_lines(&written),
            sorted_lines(&std::fs::read_to_string(&golden).unwrap())
        );
    }

    // [spec:kbdgen:req:pbxproj.model.read+1/test]
    #[cfg(target_os = "macos")]
    #[test]
    fn plutil_failure_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.pbxproj");
        std::fs::write(&path, "garbage {").unwrap();

        let error = Pbxproj::from_path(&path).unwrap_err().to_string();

        assert!(
            error.starts_with(&format!(
                "`plutil -convert json {}` failed with exit status: 1: ",
                path.display()
            )),
            "{error}"
        );
    }
}
