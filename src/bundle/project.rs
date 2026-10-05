use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

// [spec:kbdgen:def:bundle.project]
#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub locales: IndexMap<String, LocaleProjectDescription>,
    pub author: String,
    pub copyright: String,
    pub email: String,
    pub organisation: String,
    #[serde(default)]
    pub dependencies: IndexMap<String, Dependency>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dependency {
    pub url: String,
    pub layouts: Vec<String>,
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocaleProjectDescription {
    pub name: String,
    pub description: String,
}

#[cfg(test)]
mod tests {
    use crate::bundle::{Error, KbdgenBundle, fixture, read_kbdgen_bundle};

    const COMPLETE: &str = "\
locales:
  en:
    name: Test
    description: Test
author: Author
copyright: Copyright
email: test@example.com
organisation: Organisation
";

    fn load_with_project(project: &str) -> Result<KbdgenBundle, Error> {
        let root = tempfile::tempdir().unwrap();
        let path = fixture::write_bundle(root.path(), "sme", &[], &[], &[]);
        fixture::write_file(&path, "project.yaml", project);
        read_kbdgen_bundle(&path)
    }

    fn yaml_error_message(result: Result<KbdgenBundle, Error>) -> String {
        match result {
            Err(Error::Yaml(_, e)) => e.inner().to_string(),
            other => panic!("expected a YAML error, got {other:?}"),
        }
    }

    // [spec:kbdgen:def:bundle.project/test]
    #[test]
    fn each_missing_required_field_is_an_error() {
        for field in ["locales", "author", "copyright", "email", "organisation"] {
            let project: String = COMPLETE
                .split_inclusive('\n')
                .scan(false, |skipping, line| {
                    if line.starts_with(field) {
                        *skipping = true;
                    } else if !line.starts_with(' ') {
                        *skipping = false;
                    }
                    Some(if *skipping { "" } else { line })
                })
                .collect();

            let message = yaml_error_message(load_with_project(&project));

            assert!(
                message.contains(&format!("missing field `{field}`")),
                "{field}: {message}"
            );
        }
    }

    // [spec:kbdgen:def:bundle.project/test]
    #[test]
    fn locale_requires_name_and_description() {
        for (field, locale) in [
            ("name", "    description: Test\n"),
            ("description", "    name: Test\n"),
        ] {
            let project = COMPLETE.replace("    name: Test\n    description: Test\n", locale);

            let message = yaml_error_message(load_with_project(&project));

            assert!(
                message.contains(&format!("missing field `{field}`")),
                "{field}: {message}"
            );
        }
    }

    // [spec:kbdgen:def:bundle.project/test]
    #[test]
    fn dependencies_default_to_empty() {
        let bundle = load_with_project(COMPLETE).unwrap();

        assert!(bundle.project.dependencies.is_empty());
        assert_eq!(bundle.project.author, "Author");
        assert_eq!(bundle.project.organisation, "Organisation");
    }

    // [spec:kbdgen:def:bundle.project/test]
    #[test]
    fn project_maps_keep_order_and_accept_scalars() {
        let project = "\
locales:
  se:
    name: Sámi
    description: 1.5
  en:
    name: English
    description: true
author: Author
copyright: 2024
email: test@example.com
organisation: Organisation
unknownField: ignored
dependencies:
  sme:
    url: giellalt/keyboard-sme
    layouts: [se, se-FI]
  sma:
    url: giellalt/keyboard-sma
    layouts: [sma]
    branch: feature/x
";

        let project = load_with_project(project).unwrap().project;

        assert_eq!(project.copyright, "2024");
        let locales: Vec<_> = project
            .locales
            .iter()
            .map(|(k, v)| (k.as_str(), v.name.as_str(), v.description.as_str()))
            .collect();
        assert_eq!(locales, [("se", "Sámi", "1.5"), ("en", "English", "true")]);
        let ids: Vec<&str> = project.dependencies.keys().map(String::as_str).collect();
        assert_eq!(ids, ["sme", "sma"]);
        let sme = &project.dependencies["sme"];
        assert_eq!(sme.url, "giellalt/keyboard-sme");
        assert_eq!(sme.layouts, ["se", "se-FI"]);
        assert_eq!(sme.branch, None);
        assert_eq!(
            project.dependencies["sma"].branch.as_deref(),
            Some("feature/x")
        );
    }
}
