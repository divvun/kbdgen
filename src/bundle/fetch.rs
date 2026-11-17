use std::path::Path;

use anyhow::{Context, bail};

use super::project::Project;

// [spec:kbdgen:req:bundle.fetch+1]
pub async fn fetch(target: &Path, project: &Project) -> anyhow::Result<()> {
    let layouts_dir = target.join("layouts");
    let projects_dir = target.join("projects");
    std::fs::create_dir_all(&layouts_dir)
        .with_context(|| format!("create {}", layouts_dir.display()))?;
    std::fs::create_dir_all(&projects_dir)
        .with_context(|| format!("create {}", projects_dir.display()))?;

    for (id, bundle) in project.dependencies.iter() {
        let Some(repository) = bundle.url.split('/').nth(1) else {
            bail!(
                "{id}: url {:?} is not a GitHub <owner>/<repository>",
                bundle.url
            );
        };
        let branch = bundle.branch.as_deref().unwrap_or("main");
        let url = format!("https://github.com/{}/archive/{}.zip", bundle.url, branch);

        let tempdir = tempfile::tempdir()?;

        tracing::info!("Downloading {id} from {url}");
        let response = reqwest::get(&url)
            .await
            .and_then(reqwest::Response::error_for_status)
            .with_context(|| format!("{id}: download {url}"))?;
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("{id}: download {url}"))?;
        let mut zipfile = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .with_context(|| format!("{id}: {url} is not a zip archive"))?;

        tracing::debug!("Extracting {id} to {}", tempdir.path().display());
        zipfile
            .extract(tempdir.path())
            .with_context(|| format!("{id}: extract {url}"))?;

        let kbdgen_path = tempdir
            .path()
            .join(format!("{}-{}", repository, branch.replace('/', "-")))
            .join(format!("{id}.kbdgen"));

        for layout in &bundle.layouts {
            copy(
                &kbdgen_path.join("layouts").join(format!("{layout}.yaml")),
                &layouts_dir.join(format!("{layout}.yaml")),
            )?;
            copy(
                &kbdgen_path.join("project.yaml"),
                &projects_dir.join(format!("{layout}.yaml")),
            )?;
        }
    }

    Ok(())
}

fn copy(from: &Path, to: &Path) -> anyhow::Result<()> {
    tracing::debug!("Copying {} to {}", from.display(), to.display());
    std::fs::copy(from, to)
        .with_context(|| format!("copy {} to {}", from.display(), to.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;

    #[tokio::test]
    async fn fetch_without_dependencies_creates_directories() {
        let root = tempfile::tempdir().unwrap();
        let project = Project {
            locales: IndexMap::new(),
            author: "Test".to_string(),
            copyright: "Test".to_string(),
            email: "test@example.com".to_string(),
            organisation: "Test".to_string(),
            dependencies: IndexMap::new(),
        };

        fetch(root.path(), &project).await.unwrap();

        assert!(root.path().join("layouts").is_dir());
        assert!(root.path().join("projects").is_dir());
        assert_eq!(
            std::fs::read_dir(root.path().join("projects"))
                .unwrap()
                .count(),
            0
        );
    }

    // [spec:kbdgen:req:bundle.fetch+1/test]
    #[tokio::test]
    async fn a_url_without_a_repository_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let project = Project {
            locales: IndexMap::new(),
            author: "Test".to_string(),
            copyright: "Test".to_string(),
            email: "test@example.com".to_string(),
            organisation: "Test".to_string(),
            dependencies: IndexMap::from([(
                "sme".to_string(),
                crate::bundle::project::Dependency {
                    url: "giellalt".to_string(),
                    branch: None,
                    layouts: vec!["se".to_string()],
                },
            )]),
        };

        let error = fetch(root.path(), &project).await.unwrap_err();

        assert_eq!(
            error.to_string(),
            "sme: url \"giellalt\" is not a GitHub <owner>/<repository>"
        );
    }

    // [spec:kbdgen:req:bundle.fetch+1/test]
    #[tokio::test]
    async fn https_urls_reach_the_socket() {
        // A client compiled without TLS rejects an https:// URL before it opens
        // a socket, so the refusal from a port nothing listens on shows that
        // the client can speak TLS.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let client = reqwest::Client::builder().no_proxy().build().unwrap();

        let error = client
            .get(format!("https://127.0.0.1:{port}/"))
            .send()
            .await
            .unwrap_err();

        let mut cause = std::error::Error::source(&error);
        let mut refused = false;
        while let Some(inner) = cause {
            if let Some(io) = inner.downcast_ref::<std::io::Error>() {
                refused |= io.kind() == std::io::ErrorKind::ConnectionRefused;
            }
            cause = inner.source();
        }
        assert!(refused, "{error:?}");
    }
}
