use crate::build::ios::pbxproj::*;

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
            if let Some(_k) = key {
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

fn list_to_pbxproj_string<T: AsRef<str>>(item_iter: impl Iterator<Item = T>) -> String {
    let mut item_iter = item_iter.peekable();
    let mut item_string: String = String::new();
    while let Some(item) = item_iter.next() {
        item_string.push_str(&format!("\n\t\t\t\t{} /* {} */", item.as_ref(), "TODO"));
        item_string.push_str(",");
    }

    item_string
}

impl Pbxproj {
    pub fn to_pbxproj_string(&self) -> String {
        tracing::debug!("Started serializing pbxproj from json back to original format");

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
                    let mut settings_string: String = String::new();
                    values.as_array().into_iter().for_each(|value_list| {
                        settings_string.push_str("(");
                        for value in value_list {
                            settings_string.push_str(&format!("{}, ", value.as_str().unwrap()));
                        }
                        settings_string.push_str(")");
                    });
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
            s.push_str(&format!(
                "\t\t\tfiles = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(frameworks_build_phase.files.clone().into_iter())
            ));
            s.push_str(&format!(
                "\t\t\trunOnlyForDeploymentPostprocessing = {};\n",
                frameworks_build_phase.run_only_for_deployment_postprocessing
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXFrameworksBuildPhase section */\n\n");
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

            s.push_str(&format!(
                "\t\t\tchildren = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(group.children.clone().into_iter())
            ));
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
            s.push_str(&format!(
                "\t\t\tfiles = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(headers_build_phase.files.clone().into_iter())
            ));
            s.push_str(&format!(
                "\t\t\trunOnlyForDeploymentPostprocessing = {};\n",
                headers_build_phase.run_only_for_deployment_postprocessing
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXHeadersBuildPhase section */\n\n");
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
            s.push_str(&format!(
                "\t\t\tbuildPhases = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(native_target.build_phases.clone().into_iter())
            ));
            s.push_str(&format!(
                "\t\t\tbuildRules = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(native_target.build_rules.clone().into_iter())
            ));
            s.push_str(&format!(
                "\t\t\tdependencies = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(native_target.dependencies.clone().into_iter())
            ));
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
            s.push_str(&format!(
                "\t\t\tknownRegions = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(project.known_regions.clone().into_iter())
            ));
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
                list_to_pbxproj_string(project.targets.iter())
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXProject section */\n\n");
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
            s.push_str(&format!(
                "\t\t\tfiles = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(resources_build_phase.files.clone().into_iter())
            ));
            s.push_str(&format!(
                "\t\t\trunOnlyForDeploymentPostprocessing = {};\n",
                resources_build_phase.run_only_for_deployment_postprocessing
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXResourcesBuildPhase section */\n\n");
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
            s.push_str(&format!(
                "\t\t\tfiles = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(shell_script_build_phase.files.clone().into_iter())
            ));
            if let Some(input_list_paths) = shell_script_build_phase.input_file_list_paths.as_ref()
            {
                s.push_str(&format!(
                    "\t\t\tinputFileListPaths = ({}\n\t\t\t);\n",
                    list_to_pbxproj_string(input_list_paths.clone().into_iter())
                ));
            }
            {
                let mut item_string: String = String::new();
                for item in shell_script_build_phase.input_paths.clone().into_iter() {
                    item_string.push_str(&format!("\n\t\t\t\t\"{}\"", item.as_str()));
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
                s.push_str(&format!(
                    "\t\t\toutputFileListPaths = ({}\n\t\t\t);\n",
                    list_to_pbxproj_string(output_list_paths.clone().into_iter())
                ));
            }
            {
                let mut item_string: String = String::new();
                for item in shell_script_build_phase.output_paths.clone().into_iter() {
                    item_string.push_str(&format!("\n\t\t\t\t\"{}\"", item.as_str()));
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
            s.push_str(&format!(
                "\t\t\tfiles = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(sources_build_phase.files.clone().into_iter())
            ));
            s.push_str(&format!(
                "\t\t\trunOnlyForDeploymentPostprocessing = {};\n",
                sources_build_phase.run_only_for_deployment_postprocessing
            ));
            s.push_str("\t\t};\n");
        }
        s.push_str("/* End PBXSourcesBuildPhase section */\n\n");
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
        // END PBXTargetDependency

        // START PBXVariantGroup
        s.push_str("/* Start PBXVariantGroup section */\n\n");
        for (oid, variant_group) in iter_object!(self, VariantGroup) {
            s.push_str(&format!("\t\t{} /* {} */ = {{\n", oid, "TODO"));
            s.push_str("\t\t\tisa = PBXVariantGroup;\n");
            s.push_str(&format!(
                "\t\t\tchildren = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(variant_group.children.clone().into_iter())
            ));
            if let Some(name) = variant_group.name.as_ref() {
                s.push_str(&format!("\t\t\tname = {:?};\n", name));
            }
            if let Some(path) = variant_group.path.as_ref() {
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
                            let new_key;
                            if item.0.contains('[') {
                                new_key = format!("\"{}\"", item.0);
                            } else {
                                new_key = format!("{}", item.0);
                            }
                            let new_value = format!("{:?}", x);

                            item_string
                                .push_str(&format!("\n\t\t\t\t{} = {};", new_key, new_value));
                        }
                        serde_json::Value::Array(a) => {
                            let mut sub_item_string: String = String::new();
                            for thing in a {
                                sub_item_string.push_str(&format!("\n\t\t\t\t{}", thing));
                                sub_item_string.push_str(",");
                            }
                            item_string.push_str(&format!(
                                "\n\t\t\t{} = ({}\n\t\t\t);\n",
                                item.0, sub_item_string
                            ));
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

            s.push_str(&format!(
                "\t\t\tbuildConfigurations = ({}\n\t\t\t);\n",
                list_to_pbxproj_string(configuration_list.build_configurations.clone().into_iter())
            ));
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

        tracing::debug!("Finished serializing pbxproj");

        return s;
    }
}
