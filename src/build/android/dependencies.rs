use std::io::Write;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use serde_json::Value;

// [spec:kbdgen:req:android.dependencies.jnilibs+1]
pub async fn install_android_deps(main_path: &Path) -> Result<()> {
    tracing::info!(
        "Installing Android dependencies into {}",
        main_path.display()
    );
    download_and_extract_jnilibs("divvun", "divvunspell", "android-jnilibs", main_path).await?;
    download_and_extract_jnilibs("divvun", "pahkat", "android", main_path).await
}

// [spec:kbdgen:req:android.dependencies.jnilibs+1]
async fn download_and_extract_jnilibs(
    org: &str,
    repo: &str,
    filter: &str,
    main_path: &Path,
) -> Result<()> {
    let archive = tempfile::NamedTempFile::new().context("create a temporary file")?;
    let asset_name = download_asset_to_file(org, repo, filter, archive.path()).await?;

    let output = Command::new("tar")
        .arg("-xzf")
        .arg(archive.path())
        .arg("-C")
        .arg(main_path)
        .output()
        .context("`tar` could not be started")?;
    if !output.status.success() {
        bail!(
            "`tar -xzf` failed to extract {asset_name} into {} with {}: {}",
            main_path.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    tracing::debug!("Extracted {asset_name} into {}", main_path.display());
    Ok(())
}

// [spec:kbdgen:req:android.dependencies.jnilibs+1]
async fn download_asset_to_file(
    org: &str,
    repo: &str,
    target_filter: &str,
    file_path: &Path,
) -> Result<String> {
    // Different release trains are cut for this repo (e.g. CLI-only vs library
    // releases), so whichever one was published most recently may not carry the
    // asset we need. Walk releases newest-first until we find one that does,
    // rather than assuming GitHub's notion of "latest" has it.
    let url = format!("https://api.github.com/repos/{org}/{repo}/releases?per_page=100");
    let list_context = || format!("list the releases of {org}/{repo}");
    let client = reqwest::Client::new();
    let releases: Value = client
        .get(&url)
        .header("User-Agent", "kbdgen-rust-client")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .with_context(list_context)?
        .json()
        .await
        .with_context(list_context)?;

    let target_filter_lower = target_filter.to_lowercase();

    let (asset_name, asset) = releases
        .as_array()
        .with_context(|| format!("the release list of {org}/{repo} is not a JSON array"))?
        .iter()
        .filter(|release| {
            !release["draft"].as_bool().unwrap_or(false)
                && !release["prerelease"].as_bool().unwrap_or(false)
        })
        .find_map(|release| {
            release["assets"].as_array()?.iter().find_map(|asset| {
                let name = asset["name"].as_str()?;
                name.to_lowercase()
                    .contains(&target_filter_lower)
                    .then_some((name, asset))
            })
        })
        .with_context(|| {
            format!("no {target_filter} asset found in any release for {org}/{repo}")
        })?;

    let download_url = asset["browser_download_url"]
        .as_str()
        .with_context(|| format!("{asset_name} in {org}/{repo} has no download URL"))?;

    tracing::info!("Downloading {asset_name} from {org}/{repo}");

    let download_context = || format!("download {download_url}");
    let response = client
        .get(download_url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .with_context(download_context)?;

    let mut file = std::fs::File::create(file_path)
        .with_context(|| format!("create {}", file_path.display()))?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(download_context)?;
        file.write_all(&chunk)
            .with_context(|| format!("write {}", file_path.display()))?;
    }

    Ok(asset_name.to_owned())
}
