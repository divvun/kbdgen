use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

pub async fn install_android_deps() -> Result<()> {
    println!("Installing Android dependencies from GitHub releases...");

    let cache_dir = github_cache_dir();
    std::fs::create_dir_all(&cache_dir)?;

    // Download divvunspell .so files for both architectures
    download_so_file("divvun", "divvunspell", "aarch64-linux-android",
                     &cache_dir.join("libdivvunspell-arm64-v8a.so")).await?;
    download_so_file("divvun", "divvunspell", "armv7-linux-androideabi",
                     &cache_dir.join("libdivvunspell-armeabi-v7a.so")).await?;

    // Download pahkat jniLibs structure
    download_jnilibs("divvun", "pahkat", &cache_dir).await?;

    Ok(())
}

pub fn github_cache_dir() -> PathBuf {
    let kbdgen_data = pathos::user::app_data_dir("kbdgen").unwrap();
    kbdgen_data.join("github-cache")
}

async fn download_so_file(org: &str, repo: &str, target_triple: &str, output_path: &PathBuf) -> Result<()> {
    let (asset_name, bytes) = download_asset(org, repo, target_triple).await?;

    // Extract .so file directly to output path
    extract_so_file(&bytes, &asset_name, output_path)?;

    println!("Saved {} to {}", asset_name, output_path.display());
    Ok(())
}

async fn download_jnilibs(org: &str, repo: &str, cache_dir: &PathBuf) -> Result<()> {
    let (asset_name, bytes) = download_asset(org, repo, "android").await?;

    let jnilibs_dir = cache_dir.join("pahkat-jniLibs");
    std::fs::create_dir_all(&jnilibs_dir)?;

    // Extract jniLibs structure
    extract_tar_gz(&bytes, &jnilibs_dir)?;

    println!("Extracted {} to {}", asset_name, jnilibs_dir.display());
    Ok(())
}

async fn download_asset(org: &str, repo: &str, target_filter: &str) -> Result<(String, Vec<u8>)> {
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
    let bytes = response.bytes().await?.to_vec();

    Ok((asset_name.to_string(), bytes))
}

fn extract_so_file(bytes: &[u8], asset_name: &str, output_path: &PathBuf) -> Result<()> {
    if asset_name.ends_with(".zip") {
        extract_so_from_zip(bytes, output_path)
    } else if asset_name.ends_with(".tar.gz") || asset_name.ends_with(".tgz") {
        extract_so_from_tar_gz(bytes, output_path)
    } else {
        Err(anyhow::anyhow!("Unsupported archive format: {}", asset_name))
    }
}

fn extract_so_from_zip(bytes: &[u8], output_path: &PathBuf) -> Result<()> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let file_path = std::path::Path::new(file.name());

        if !file.name().ends_with('/') {
            if let Some(filename) = file_path.file_name() {
                if let Some(filename_str) = filename.to_str() {
                    if filename_str.ends_with(".so") {
                        let mut outfile = std::fs::File::create(output_path)?;
                        std::io::copy(&mut file, &mut outfile)?;
                        return Ok(());
                    }
                }
            }
        }
    }
    Err(anyhow::anyhow!("No .so file found in archive"))
}

fn extract_so_from_tar_gz(bytes: &[u8], output_path: &PathBuf) -> Result<()> {
    use flate2::read::GzDecoder;
    use std::io::Cursor;
    use tar::Archive;

    let cursor = Cursor::new(bytes);
    let gz = GzDecoder::new(cursor);
    let mut archive = Archive::new(gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        if !entry.header().entry_type().is_dir() {
            if let Some(filename) = path.file_name() {
                if let Some(filename_str) = filename.to_str() {
                    if filename_str.ends_with(".so") {
                        entry.unpack(output_path)?;
                        return Ok(());
                    }
                }
            }
        }
    }
    Err(anyhow::anyhow!("No .so file found in archive"))
}

fn extract_tar_gz(bytes: &[u8], extract_dir: &PathBuf) -> Result<()> {
    use flate2::read::GzDecoder;
    use std::io::Cursor;
    use tar::Archive;

    let cursor = Cursor::new(bytes);
    let gz = GzDecoder::new(cursor);
    let mut archive = Archive::new(gz);
    archive.unpack(extract_dir)?;
    Ok(())
}
