use anyhow::Result;
use futures_util::StreamExt;
use serde_json::Value;
use std::io::Write;
use std::path::Path;

pub async fn install_android_deps(jni_libs_path: &Path) -> Result<()> {
    println!("Installing Android dependencies from GitHub releases...");
    download_and_extract_jnilibs("divvun", "divvunspell", "android-jnilibs", jni_libs_path).await?;
    download_and_extract_jnilibs("divvun", "pahkat", "android", jni_libs_path).await?;
    Ok(())
}

async fn download_and_extract_jnilibs(
    org: &str,
    repo: &str,
    filter: &str,
    jni_libs_path: &Path,
) -> Result<()> {
    let temp_file = tempfile::NamedTempFile::new()?;
    let asset_name = download_asset_to_file(org, repo, filter, temp_file.path()).await?;

    let status = std::process::Command::new("tar")
        .args(&[
            "-xzf",
            temp_file
                .path()
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid temp file path"))?,
            "-C",
            jni_libs_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid jniLibs path"))?,
        ])
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Failed to extract {} with tar", asset_name));
    }

    println!("Extracted {} to {}", asset_name, jni_libs_path.display());
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
        .ok_or_else(|| anyhow::anyhow!("No {} asset found for {}/{}", target_filter, org, repo))?;

    let download_url = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No download URL found"))?;

    let asset_name = asset["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No asset name found"))?;

    println!("Downloading {}", asset_name);

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
