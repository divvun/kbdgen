use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Display, Write},
};

use indexmap::IndexMap;
use serde_json::Value;

use crate::build::ios::pbxproj::*;

/// The comment written after object ids that kbdgen does not name. Comments are ignored when a
/// project is read, and Xcode writes its own the next time it saves the project.
const PLACEHOLDER: &str = "TODO";

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

/// How a section's opening line reads. Xcode writes `Begin`; most of kbdgen's sections are
/// opened with `Start` and an empty line instead, which Xcode reads just the same.
enum Opening {
    Begin,
    Start,
}

fn section<'a, T: 'a>(
    out: &mut String,
    isa: &str,
    opening: Opening,
    objects: impl Iterator<Item = (&'a ObjectId, &'a T)>,
    mut write_object: impl FnMut(&mut String, &ObjectId, &T) -> fmt::Result,
) -> fmt::Result {
    match opening {
        Opening::Begin => writeln!(out, "/* Begin {isa} section */")?,
        Opening::Start => write!(out, "/* Start {isa} section */\n\n")?,
    }
    for (id, object) in objects {
        write_object(out, id, object)?;
    }
    write!(out, "/* End {isa} section */\n\n")
}

fn open_object(out: &mut String, id: &ObjectId, comment: Option<&str>, isa: &str) -> fmt::Result {
    match comment {
        Some(comment) => writeln!(out, "\t\t{id} /* {comment} */ = {{")?,
        None => writeln!(out, "\t\t{id} = {{")?,
    }
    writeln!(out, "\t\t\tisa = {isa};")
}

fn close_object(out: &mut String) -> fmt::Result {
    writeln!(out, "\t\t}};")
}

fn field(out: &mut String, key: &str, value: impl Display) -> fmt::Result {
    writeln!(out, "\t\t\t{key} = {value};")
}

fn quoted_field(out: &mut String, key: &str, value: &str) -> fmt::Result {
    writeln!(out, "\t\t\t{key} = {value:?};")
}

// [spec:kbdgen:syn:pbxproj.serialize+2]
fn list<T: Display>(
    out: &mut String,
    key: &str,
    items: impl IntoIterator<Item = T>,
) -> fmt::Result {
    write!(out, "\t\t\t{key} = (")?;
    for item in items {
        write!(out, "\n\t\t\t\t{item} /* {PLACEHOLDER} */,")?;
    }
    writeln!(out, "\n\t\t\t);")
}

fn path_list(out: &mut String, key: &str, items: &BTreeSet<String>) -> fmt::Result {
    write!(out, "\t\t\t{key} = (")?;
    for item in items {
        write!(out, "\n\t\t\t\t\"{item}\",")?;
    }
    writeln!(out, "\n\t\t\t);")
}

fn quoted_if(value: &str, quote: bool) -> Cow<'_, str> {
    if quote {
        Cow::Owned(format!("{value:?}"))
    } else {
        Cow::Borrowed(value)
    }
}

fn source_tree(tree: &str) -> &str {
    if tree == "<group>" {
        "\"<group>\""
    } else {
        tree
    }
}

fn build_phase(
    out: &mut String,
    id: &ObjectId,
    isa: &str,
    build_action_mask: &str,
    files: &BTreeSet<ObjectId>,
    run_only_for_deployment_postprocessing: &str,
) -> fmt::Result {
    open_object(out, id, Some(PLACEHOLDER), isa)?;
    field(out, "buildActionMask", build_action_mask)?;
    list(out, "files", files)?;
    field(
        out,
        "runOnlyForDeploymentPostprocessing",
        run_only_for_deployment_postprocessing,
    )?;
    close_object(out)
}

fn indent(out: &mut String, depth: usize) {
    out.extend(std::iter::repeat_n('\t', depth));
}

// [spec:kbdgen:req:pbxproj.serialize.attributes+1]
fn write_attributes(
    out: &mut String,
    attributes: &serde_json::Map<String, Value>,
    depth: usize,
) -> fmt::Result {
    let mut entries: Vec<(&String, &Value)> = attributes.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    for (key, value) in entries {
        indent(out, depth);
        write!(out, "{key} = ")?;
        match value {
            Value::String(value) => writeln!(out, "{value};")?,
            Value::Object(children) => {
                out.push_str("{\n");
                write_attributes(out, children, depth + 1)?;
                indent(out, depth);
                out.push_str("};\n");
            }
            Value::Array(items) => {
                out.push_str("(\n");
                for item in items {
                    let Value::String(item) = item else {
                        panic!("project attribute {key} has a non-string item {item}");
                    };
                    indent(out, depth + 1);
                    writeln!(out, "{item},")?;
                }
                indent(out, depth);
                out.push_str(");\n");
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {
                panic!("project attribute {key} is {value}, which plutil never produces")
            }
        }
    }
    Ok(())
}

fn write_build_file_settings(out: &mut String, settings: &BTreeMap<String, Value>) -> fmt::Result {
    out.push_str("settings = {");
    for (key, value) in settings {
        match value {
            Value::String(value) => write!(out, "{key} = {value:?};")?,
            Value::Array(items) => {
                write!(out, "{key} = (")?;
                for item in items {
                    let Value::String(item) = item else {
                        panic!("build file setting {key} has a non-string item {item}");
                    };
                    write!(out, "{item}, ")?;
                }
                out.push_str(");");
            }
            other => panic!("build file setting {key} has an unsupported value {other}"),
        }
    }
    out.push_str("};");
    Ok(())
}

// [spec:kbdgen:req:pbxproj.serialize.build-settings+1]
fn write_build_settings(out: &mut String, settings: &IndexMap<String, Value>) -> fmt::Result {
    out.push_str("\n\t\t\tbuildSettings = {");
    for (key, value) in settings {
        match value {
            Value::String(value) => {
                let key = if key.contains('[') {
                    Cow::Owned(format!("\"{key}\""))
                } else {
                    Cow::Borrowed(key.as_str())
                };
                write!(out, "\n\t\t\t\t{key} = {value:?};")?;
            }
            Value::Array(items) => {
                write!(out, "\n\t\t\t{key} = (")?;
                for item in items {
                    write!(out, "\n\t\t\t\t{item},")?;
                }
                out.push_str("\n\t\t\t);\n");
            }
            other => panic!("build setting {key} has an unsupported value {other}"),
        }
    }
    out.push_str("\n\t\t\t};\n");
    Ok(())
}

impl Pbxproj {
    pub fn to_pbxproj_string(&self) -> String {
        let mut out = String::new();
        self.write_pbxproj(&mut out)
            .expect("writing to a String cannot fail");
        out
    }

    // [spec:kbdgen:syn:pbxproj.serialize+2]
    // [spec:kbdgen:def:pbxproj.serialize.fields+2]
    fn write_pbxproj(&self, out: &mut String) -> fmt::Result {
        out.push_str("// !$*UTF8*$!\n{\n");
        writeln!(out, "\tarchiveVersion = {};", self.archive_version)?;
        out.push_str("\tclasses = {\n\t};\n");
        writeln!(out, "\tobjectVersion = {};", self.object_version)?;
        out.push_str("\tobjects = {\n\n");

        section(
            out,
            "PBXBuildFile",
            Opening::Begin,
            iter_object!(self, BuildFile),
            |out, id, build_file| {
                write!(out, "\t\t{id} /* {PLACEHOLDER} */ = {{isa = PBXBuildFile; ")?;
                if let Some(file_ref) = &build_file.file_ref {
                    write!(out, "fileRef = {file_ref} /* {PLACEHOLDER} */; ")?;
                }
                if let Some(product_ref) = &build_file.product_ref {
                    write!(out, "productRef = {product_ref} /* {PLACEHOLDER} */; ")?;
                }
                if let Some(settings) = &build_file.settings {
                    write_build_file_settings(out, settings)?;
                }
                out.push_str("};\n");
                Ok(())
            },
        )?;

        section(
            out,
            "PBXContainerItemProxy",
            Opening::Begin,
            iter_object!(self, ContainerItemProxy),
            |out, id, proxy| {
                open_object(
                    out,
                    id,
                    Some("PBXContainerItemProxy"),
                    "PBXContainerItemProxy",
                )?;
                field(
                    out,
                    "containerPortal",
                    format_args!("{} /* {PLACEHOLDER} */", proxy.container_portal),
                )?;
                field(out, "proxyType", &proxy.proxy_type)?;
                field(out, "remoteGlobalIDString", &proxy.remote_global_id_string)?;
                field(out, "remoteInfo", &proxy.remote_info)?;
                close_object(out)
            },
        )?;

        section(
            out,
            "PBXCopyFilesBuildPhase",
            Opening::Begin,
            iter_object!(self, CopyFilesBuildPhase),
            |out, id, phase| {
                open_object(out, id, Some(PLACEHOLDER), "PBXCopyFilesBuildPhase")?;
                field(out, "buildActionMask", &phase.build_action_mask)?;
                quoted_field(out, "dstPath", &phase.dst_path)?;
                field(out, "dstSubfolderSpec", &phase.dst_subfolder_spec)?;
                list(out, "files", &phase.files)?;
                if let Some(name) = &phase.name {
                    quoted_field(out, "name", name)?;
                }
                field(
                    out,
                    "runOnlyForDeploymentPostprocessing",
                    &phase.run_only_for_deployment_postprocessing,
                )?;
                close_object(out)
            },
        )?;

        section(
            out,
            "PBXFileReference",
            Opening::Start,
            iter_object!(self, FileReference),
            |out, id, file_ref| {
                let comment = file_ref.name.as_deref().unwrap_or(&file_ref.path);
                write!(out, "\t\t{id} /* {comment} */ = {{isa = PBXFileReference; ")?;
                if let Some(encoding) = &file_ref.file_encoding {
                    write!(out, "fileEncoding = {encoding}; ")?;
                }
                if let Some(include) = &file_ref.include_in_index {
                    write!(out, "includeInIndex = {include}; ")?;
                }
                if let Some(file_type) = &file_ref.last_known_file_type {
                    write!(out, "lastKnownFileType = {file_type}; ")?;
                }
                // [spec:kbdgen:req:pbxproj.quoting+2]
                if let Some(name) = &file_ref.name {
                    write!(
                        out,
                        "name = {}; ",
                        quoted_if(name, name.contains(['-', '+']))
                    )?;
                }
                // [spec:kbdgen:req:pbxproj.quoting+2]
                let path = &file_ref.path;
                write!(
                    out,
                    "path = {}; ",
                    quoted_if(path, path.contains(['-', '+']))
                )?;
                write!(out, "sourceTree = {}; ", source_tree(&file_ref.source_tree))?;
                out.push_str("\t\t};\n");
                Ok(())
            },
        )?;

        section(
            out,
            "PBXFrameworksBuildPhase",
            Opening::Start,
            iter_object!(self, FrameworksBuildPhase),
            |out, id, phase| {
                build_phase(
                    out,
                    id,
                    "PBXFrameworksBuildPhase",
                    &phase.build_action_mask,
                    &phase.files,
                    &phase.run_only_for_deployment_postprocessing,
                )
            },
        )?;

        section(
            out,
            "PBXGroup",
            Opening::Start,
            iter_object!(self, Group),
            |out, id, group| {
                let comment = group.name.as_deref().or(group.path.as_deref());
                open_object(out, id, comment, "PBXGroup")?;
                list(out, "children", &group.children)?;
                // [spec:kbdgen:req:pbxproj.quoting+2]
                if let Some(path) = &group.path {
                    field(out, "path", quoted_if(path, path.contains(' ')))?;
                }
                // [spec:kbdgen:req:pbxproj.quoting+2]
                if let Some(name) = &group.name {
                    field(out, "name", quoted_if(name, name.contains(' ')))?;
                }
                field(out, "sourceTree", source_tree(&group.source_tree))?;
                close_object(out)
            },
        )?;

        section(
            out,
            "PBXHeadersBuildPhase",
            Opening::Start,
            iter_object!(self, HeadersBuildPhase),
            |out, id, phase| {
                build_phase(
                    out,
                    id,
                    "PBXHeadersBuildPhase",
                    &phase.build_action_mask,
                    &phase.files,
                    &phase.run_only_for_deployment_postprocessing,
                )
            },
        )?;

        section(
            out,
            "PBXNativeTarget",
            Opening::Start,
            iter_object!(self, NativeTarget),
            |out, id, target| {
                open_object(out, id, Some(PLACEHOLDER), "PBXNativeTarget")?;
                field(
                    out,
                    "buildConfigurationList",
                    &target.build_configuration_list,
                )?;
                list(out, "buildPhases", &target.build_phases)?;
                list(out, "buildRules", &target.build_rules)?;
                list(out, "dependencies", &target.dependencies)?;
                quoted_field(out, "name", &target.name)?;
                if let Some(dependencies) = &target.package_product_dependencies {
                    list(out, "packageProductDependencies", dependencies)?;
                }
                field(out, "productName", &target.product_name)?;
                if let Some(product) = &target.product_reference {
                    field(
                        out,
                        "productReference",
                        format_args!("{product} /* {PLACEHOLDER} */"),
                    )?;
                }
                if let Some(product_type) = &target.product_type {
                    field(out, "productType", product_type)?;
                }
                close_object(out)
            },
        )?;

        section(
            out,
            "PBXProject",
            Opening::Start,
            iter_object!(self, Project),
            |out, id, project| {
                open_object(out, id, Some("Project object"), "PBXProject")?;
                out.push_str("\t\t\tattributes = {\n");
                write_attributes(out, &project.attributes, 4)?;
                out.push_str("\t\t\t};\n");
                field(
                    out,
                    "buildConfigurationList",
                    format_args!(
                        "{} /* {PLACEHOLDER}: Build configuration list for PBXProject \"GiellaKeyboard\" */",
                        project.build_configuration_list
                    ),
                )?;
                quoted_field(out, "compatibilityVersion", &project.compatibility_version)?;
                field(out, "developmentRegion", &project.development_region)?;
                field(
                    out,
                    "hasScannedForEncodings",
                    &project.has_scanned_for_encodings,
                )?;
                list(out, "knownRegions", &project.known_regions)?;
                field(out, "mainGroup", &project.main_group)?;
                if let Some(package_references) = &project.package_references {
                    list(out, "packageReferences", package_references)?;
                }
                field(
                    out,
                    "productRefGroup",
                    format_args!("{} /* Products */", project.product_ref_group),
                )?;
                quoted_field(out, "projectDirPath", &project.project_dir_path)?;
                quoted_field(out, "projectRoot", &project.project_root)?;
                list(out, "targets", &project.targets)?;
                close_object(out)
            },
        )?;

        section(
            out,
            "PBXResourcesBuildPhase",
            Opening::Start,
            iter_object!(self, ResourcesBuildPhase),
            |out, id, phase| {
                build_phase(
                    out,
                    id,
                    "PBXResourcesBuildPhase",
                    &phase.build_action_mask,
                    &phase.files,
                    &phase.run_only_for_deployment_postprocessing,
                )
            },
        )?;

        section(
            out,
            "PBXShellScriptBuildPhase",
            Opening::Start,
            iter_object!(self, ShellScriptBuildPhase),
            |out, id, phase| {
                open_object(out, id, Some(PLACEHOLDER), "PBXShellScriptBuildPhase")?;
                field(out, "buildActionMask", &phase.build_action_mask)?;
                list(out, "files", &phase.files)?;
                if let Some(paths) = &phase.input_file_list_paths {
                    path_list(out, "inputFileListPaths", paths)?;
                }
                path_list(out, "inputPaths", &phase.input_paths)?;
                quoted_field(out, "name", &phase.name)?;
                if let Some(paths) = &phase.output_file_list_paths {
                    path_list(out, "outputFileListPaths", paths)?;
                }
                path_list(out, "outputPaths", &phase.output_paths)?;
                field(
                    out,
                    "runOnlyForDeploymentPostprocessing",
                    &phase.run_only_for_deployment_postprocessing,
                )?;
                field(out, "shellPath", &phase.shell_path)?;
                quoted_field(out, "shellScript", &phase.shell_script)?;
                if let Some(show) = &phase.show_env_vars_in_log {
                    field(out, "showEnvVarsInLog", show)?;
                }
                close_object(out)
            },
        )?;

        section(
            out,
            "PBXSourcesBuildPhase",
            Opening::Start,
            iter_object!(self, SourcesBuildPhase),
            |out, id, phase| {
                build_phase(
                    out,
                    id,
                    "PBXSourcesBuildPhase",
                    &phase.build_action_mask,
                    &phase.files,
                    &phase.run_only_for_deployment_postprocessing,
                )
            },
        )?;

        section(
            out,
            "PBXTargetDependency",
            Opening::Start,
            iter_object!(self, TargetDependency),
            |out, id, dependency| {
                open_object(out, id, Some(PLACEHOLDER), "PBXTargetDependency")?;
                field(out, "target", &dependency.target)?;
                field(out, "targetProxy", &dependency.target_proxy)?;
                close_object(out)
            },
        )?;

        section(
            out,
            "PBXVariantGroup",
            Opening::Start,
            iter_object!(self, VariantGroup),
            |out, id, group| {
                open_object(out, id, Some(PLACEHOLDER), "PBXVariantGroup")?;
                list(out, "children", &group.children)?;
                if let Some(name) = &group.name {
                    quoted_field(out, "name", name)?;
                }
                if let Some(path) = &group.path {
                    field(out, "path", path)?;
                }
                field(out, "sourceTree", source_tree(&group.source_tree))?;
                close_object(out)
            },
        )?;

        section(
            out,
            "XCBuildConfiguration",
            Opening::Start,
            iter_object!(self, BuildConfiguration),
            |out, id, configuration| {
                open_object(out, id, Some(PLACEHOLDER), "XCBuildConfiguration")?;
                write_build_settings(out, &configuration.build_settings)?;
                quoted_field(out, "name", &configuration.name)?;
                close_object(out)
            },
        )?;

        section(
            out,
            "XCConfigurationList",
            Opening::Start,
            iter_object!(self, ConfigurationList),
            |out, id, list_object| {
                open_object(out, id, Some(PLACEHOLDER), "XCConfigurationList")?;
                list(
                    out,
                    "buildConfigurations",
                    &list_object.build_configurations,
                )?;
                field(
                    out,
                    "defaultConfigurationIsVisible",
                    &list_object.default_configuration_is_visible,
                )?;
                field(
                    out,
                    "defaultConfigurationName",
                    &list_object.default_configuration_name,
                )?;
                close_object(out)
            },
        )?;

        section(
            out,
            "XCRemoteSwiftPackageReference",
            Opening::Begin,
            iter_object!(self, RemoteSwiftPackageReference),
            |out, id, package| {
                open_object(out, id, Some(PLACEHOLDER), "XCRemoteSwiftPackageReference")?;
                quoted_field(out, "repositoryURL", &package.repository_url)?;
                out.push_str("\t\t\trequirement = {\n");
                for (key, value) in &package.requirement {
                    writeln!(out, "\t\t\t\t{key} = {value:?};")?;
                }
                out.push_str("\t\t\t};\n");
                close_object(out)
            },
        )?;

        section(
            out,
            "XCSwiftPackageProductDependency",
            Opening::Begin,
            iter_object!(self, SwiftPackageProductDependency),
            |out, id, dependency| {
                open_object(
                    out,
                    id,
                    Some(PLACEHOLDER),
                    "XCSwiftPackageProductDependency",
                )?;
                if let Some(package) = &dependency.package {
                    field(
                        out,
                        "package",
                        format_args!("{package} /* {PLACEHOLDER} */"),
                    )?;
                }
                quoted_field(out, "productName", &dependency.product_name)?;
                close_object(out)
            },
        )?;

        out.push_str("\t};\n");
        writeln!(out, "\trootObject = {};", self.root_object)?;
        out.push_str("}\n");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Pbxproj {
        serde_json::from_value(serde_json::json!({
            "archiveVersion": "1",
            "classes": {},
            "objectVersion": "52",
            "rootObject": "P0",
            "objects": {
                "P0": {
                    "isa": "PBXProject",
                    "attributes": {
                        "knownAssetTags": ["New", "Old"],
                        "TargetAttributes": {
                            "T0": { "ProvisioningStyle": "Automatic", "CreatedOnToolsVersion": "6.0" }
                        },
                        "LastUpgradeCheck": "1130"
                    },
                    "buildConfigurationList": "CL",
                    "compatibilityVersion": "Xcode 3.2",
                    "developmentRegion": "en",
                    "hasScannedForEncodings": "0",
                    "knownRegions": ["en", "Base"],
                    "mainGroup": "G0",
                    "productRefGroup": "G1",
                    "projectDirPath": "",
                    "projectRoot": "",
                    "targets": ["T0"],
                    "packageReferences": ["R0"]
                },
                "B0": {
                    "isa": "PBXBuildFile",
                    "fileRef": "F0",
                    "settings": {
                        "ATTRIBUTES": ["CodeSignOnCopy", "RemoveHeadersOnCopy"],
                        "COMPILER_FLAGS": "-fno-objc-arc"
                    }
                },
                "B1": { "isa": "PBXBuildFile", "productRef": "D0" },
                "F0": {
                    "isa": "PBXFileReference",
                    "explicitFileType": "wrapper.app-extension",
                    "includeInIndex": "0",
                    "name": "Foo-Bar",
                    "path": "Foo-Bar.appex",
                    "sourceTree": "BUILT_PRODUCTS_DIR"
                },
                "F1": {
                    "isa": "PBXFileReference",
                    "fileEncoding": "4",
                    "lastKnownFileType": "sourcecode.swift",
                    "path": "A+B.swift",
                    "sourceTree": "<group>"
                },
                "F2": {
                    "isa": "PBXFileReference",
                    "lastKnownFileType": "text",
                    "name": "nb",
                    "path": "nb.lproj/About+More.txt",
                    "sourceTree": "<group>"
                },
                "G0": { "isa": "PBXGroup", "children": ["G1", "G2", "G3"], "sourceTree": "<group>" },
                "G1": { "isa": "PBXGroup", "children": ["F0"], "name": "Products", "sourceTree": "<group>" },
                "G2": {
                    "isa": "PBXGroup",
                    "children": ["F1"],
                    "name": "Supporting Files",
                    "path": "Hosting App",
                    "sourceTree": "<group>"
                },
                "G3": { "isa": "PBXGroup", "children": [], "path": "Keyboard", "sourceTree": "SOURCE_ROOT" },
                "V0": {
                    "isa": "PBXVariantGroup",
                    "children": ["F2"],
                    "name": "About.txt",
                    "sourceTree": "<group>"
                },
                "T0": {
                    "isa": "PBXNativeTarget",
                    "buildConfigurationList": "CL",
                    "buildPhases": ["PC", "PS", "PF", "PH", "PR", "PX"],
                    "buildRules": [],
                    "dependencies": ["TD"],
                    "name": "HostingApp",
                    "packageProductDependencies": ["D0"],
                    "productName": "HostingApp",
                    "productReference": "F0",
                    "productType": "com.apple.product-type.application"
                },
                "TD": { "isa": "PBXTargetDependency", "target": "T0", "targetProxy": "CP" },
                "CP": {
                    "isa": "PBXContainerItemProxy",
                    "containerPortal": "P0",
                    "proxyType": "1",
                    "remoteGlobalIDString": "T0",
                    "remoteInfo": "Keyboard"
                },
                "PC": {
                    "isa": "PBXCopyFilesBuildPhase",
                    "buildActionMask": "2147483647",
                    "dstPath": "",
                    "dstSubfolderSpec": "13",
                    "files": ["B0"],
                    "name": "Embed App Extensions",
                    "runOnlyForDeploymentPostprocessing": "0"
                },
                "PS": {
                    "isa": "PBXShellScriptBuildPhase",
                    "buildActionMask": "2147483647",
                    "files": [],
                    "inputFileListPaths": [],
                    "inputPaths": ["${PODS_ROOT}/Manifest.lock", "${PODS_PODFILE_DIR_PATH}/Podfile.lock"],
                    "name": "[CP] Check Pods Manifest.lock",
                    "outputFileListPaths": ["${DERIVED}/list"],
                    "outputPaths": ["$(DERIVED_FILE_DIR)/Pods-HostingApp-checkManifestLockResult.txt"],
                    "runOnlyForDeploymentPostprocessing": "0",
                    "shellPath": "/bin/sh",
                    "shellScript": "diff \"${A}\" \"${B}\"\necho \\ok\n",
                    "showEnvVarsInLog": "0"
                },
                "PF": {
                    "isa": "PBXFrameworksBuildPhase",
                    "buildActionMask": "2147483647",
                    "files": ["B1"],
                    "runOnlyForDeploymentPostprocessing": "0"
                },
                "PH": {
                    "isa": "PBXHeadersBuildPhase",
                    "buildActionMask": "2147483647",
                    "files": [],
                    "runOnlyForDeploymentPostprocessing": "0"
                },
                "PR": {
                    "isa": "PBXResourcesBuildPhase",
                    "buildActionMask": "2147483647",
                    "files": ["B0", "B1"],
                    "name": "dropped",
                    "runOnlyForDeploymentPostprocessing": "0"
                },
                "PX": {
                    "isa": "PBXSourcesBuildPhase",
                    "buildActionMask": "2147483647",
                    "files": [],
                    "runOnlyForDeploymentPostprocessing": "0"
                },
                "BC": {
                    "isa": "XCBuildConfiguration",
                    "baseConfigurationReference": "F1",
                    "buildSettings": {
                        "INFOPLIST_FILE": "Keyboard/Info.plist",
                        "CODE_SIGN_IDENTITY[sdk=iphoneos*]": "iPhone Developer",
                        "LD_RUNPATH_SEARCH_PATHS": ["$(inherited)", "@executable_path/Frameworks"],
                        "PRODUCT_NAME": "$(TARGET_NAME)"
                    },
                    "name": "Release"
                },
                "CL": {
                    "isa": "XCConfigurationList",
                    "buildConfigurations": ["BC"],
                    "defaultConfigurationIsVisible": "0",
                    "defaultConfigurationName": "Release"
                },
                "R0": {
                    "isa": "XCRemoteSwiftPackageReference",
                    "repositoryURL": "https://example.com/sentry-cocoa",
                    "requirement": { "minimumVersion": "8.0.0", "kind": "upToNextMajorVersion" }
                },
                "D0": {
                    "isa": "XCSwiftPackageProductDependency",
                    "package": "R0",
                    "productName": "Sentry"
                },
                "D1": { "isa": "XCSwiftPackageProductDependency", "productName": "Local" }
            }
        }))
        .unwrap()
    }

    // [spec:kbdgen:syn:pbxproj.serialize+2/test/golden]
    // [spec:kbdgen:def:pbxproj.serialize.fields+2/test/golden]
    // [spec:kbdgen:req:pbxproj.quoting+2/test/golden]
    // [spec:kbdgen:req:pbxproj.serialize.attributes+1/test/golden]
    // [spec:kbdgen:req:pbxproj.serialize.build-settings+1/test/golden]
    #[test]
    fn serializes_every_kind() {
        assert_eq!(
            model().to_pbxproj_string(),
            include_str!("testdata/every-kind.pbxproj")
        );
    }
}
