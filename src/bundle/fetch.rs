use std::path::Path;

use super::project::Project;

pub async fn fetch(target: &Path, project: &Project) -> anyhow::Result<()> {
    tracing::debug!("Create layouts dir");
    std::fs::create_dir_all(target.join("layouts"))?;

    tracing::debug!("Create projects dir");
    std::fs::create_dir_all(target.join("projects"))?;

    for (id, bundle) in project.dependencies.iter() {
        tracing::debug!("id: {}, bundle: {:?}", &id, &bundle);
        let branch = bundle.branch.as_deref().unwrap_or_else(|| "main".into());
        let url = format!("https://github.com/{}/archive/{}.zip", bundle.url, &branch);

        let tempdir = tempfile::tempdir()?;

        tracing::info!("Downloading {}...", id);
        let bytes = reqwest::get(url).await?.bytes().await?;
        let bytes = std::io::Cursor::new(bytes);
        let mut zipfile = zip::ZipArchive::new(bytes)?;

        tracing::info!("Unzipping {}...", id);
        zipfile.extract(tempdir.path())?;

        let kbdgen_path = tempdir
            .path()
            .join(format!(
                "{}-{}",
                bundle.url.split("/").nth(1).unwrap(),
                branch.replace("/", "-")
            ))
            .join(format!("{}.kbdgen", id));

        for layout in &bundle.layouts {
            let from_path = kbdgen_path.join("layouts").join(format!("{}.yaml", layout));
            let to_path = target.join("layouts").join(format!("{}.yaml", layout));
            tracing::info!(
                "Copying {} to {}...",
                from_path.display(),
                to_path.display()
            );
            std::fs::copy(from_path, to_path)?;

            let project_from_path = kbdgen_path.join("project.yaml");
            let project_to_path = target.join("projects").join(format!("{}.yaml", layout));
            tracing::info!("Copying {} to {}...", project_from_path.display(), project_to_path.display());
            std::fs::copy(project_from_path, project_to_path)?;
        }
    }

    Ok(())
}
