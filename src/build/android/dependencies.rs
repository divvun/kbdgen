use anyhow::Result;
use futures_util::StreamExt;
use serde_json::Value;
use std::io::Write;
use std::path::Path;

pub async fn install_android_deps(main_path: &Path) -> Result<()> {
    tracing::info!("Installing Android dependencies from GitHub releases...");
    download_and_extract_jnilibs("divvun", "divvunspell", "android-jnilibs", main_path).await?;
    download_and_extract_jnilibs("divvun", "pahkat", "android", main_path).await?;
    Ok(())
}

async fn download_and_extract_jnilibs(
    org: &str,
    repo: &str,
    filter: &str,
    main_path: &Path,
) -> Result<()> {
    let temp_file = tempfile::NamedTempFile::new()?;
    let asset_name = download_asset_to_file(org, repo, filter, temp_file.path()).await?;

    let status = std::process::Command::new("tar")
        .args(&[
            "-xzf",
            temp_file.path().to_str().expect("Valid temp file path"),
            "-C",
            main_path.to_str().expect("Valid main path"),
        ])
        .status()?;

    if !status.success() {
        panic!("Failed to extract {} with tar", asset_name);
    }

    tracing::debug!("Extracted {} to {}", asset_name, main_path.display());
    Ok(())
}

async fn download_asset_to_file(
    org: &str,
    repo: &str,
    target_filter: &str,
    file_path: &Path,
) -> Result<String> {
    let url = format!("https://api.github.com/repos/{org}/{repo}/releases/latest");
    let client = reqwest::Client::new();
    let json: Value = client
        .get(&url)
        .header("User-Agent", "kbdgen-rust-client")
        .send()
        .await?
        .json()
        .await?;

    let asset = json["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find(|asset| {
                if let Some(name) = asset["name"].as_str() {
                    let name_lower = name.to_lowercase();
                    name_lower.contains(&target_filter.to_lowercase())
                } else {
                    false
                }
            })
        })
        .expect(&format!(
            "No {} asset found for {}/{}",
            target_filter, org, repo
        ));

    let download_url = asset["browser_download_url"]
        .as_str()
        .expect("Valid asset download URL");

    let asset_name = asset["name"].as_str().expect("Valid asset name");

    tracing::debug!("Downloading {}", asset_name);

    let response = client.get(download_url).send().await?;

    let mut file = std::fs::File::create(file_path)?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
    }
    file.flush()?;

    Ok(asset_name.to_string())
}
