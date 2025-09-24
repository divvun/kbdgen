use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

pub async fn install_android_deps() -> Result<()> {
    println!("Installing Android dependencies from GitHub releases...");

    // Download both dependencies for both architectures
    download_and_extract_dependency("divvun", "divvunspell").await?;
    download_and_extract_dependency("divvun", "pahkat").await?;

    Ok(())
}

async fn download_and_extract_dependency(org: &str, repo: &str) -> Result<()> {
    if repo == "pahkat" {
        // libpahkat_client is bundled as libpahkat-android-vx.x.x.tgz with jniLibs structure
        download_latest_release(org, repo, "libpahkat-android").await?;
    } else {
        // Other dependencies like divvunspell have separate architecture builds
        download_latest_release(org, repo, "aarch64-linux-android").await?;
        download_latest_release(org, repo, "armv7-linux-androideabi").await?;
    }
    Ok(())
}

pub fn github_prefix_dir() -> PathBuf {
    let kbdgen_data = pathos::user::app_data_dir("kbdgen").unwrap();
    kbdgen_data.join("github")
}

async fn download_latest_release(org: &str, repo: &str, target_triple: &str) -> Result<()> {
    let url = format!("https://api.github.com/repos/{org}/{repo}/releases/latest");
    let client = reqwest::Client::new();
    let json: Value = client
        .get(&url)
        .header("User-Agent", "kbdgen-rust-client")
        .send()
        .await?
        .json()
        .await?;

    // Debug: print all available assets
    if let Some(assets) = json["assets"].as_array() {
        println!("Available assets for {}/{}:", org, repo);
        for asset in assets {
            if let Some(name) = asset["name"].as_str() {
                println!("  - {}", name);
            }
        }
    }

    let asset = json["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find(|asset| {
                if let Some(name) = asset["name"].as_str() {
                    let name_lower = name.to_lowercase();
                    if target_triple == "libpahkat-android" {
                        // For libpahkat, look for android in the name and .tgz extension
                        name_lower.contains("android") && name_lower.ends_with(".tgz")
                    } else {
                        // For other deps, look for exact target triple match
                        name_lower.contains(&target_triple.to_lowercase())
                    }
                } else {
                    false
                }
            })
        })
        .ok_or_else(|| anyhow::anyhow!("No {} asset found for {}/{}", target_triple, org, repo))?;

    let download_url = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No download URL found"))?;

    let asset_name = asset["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No asset name found"))?;

    println!(
        "Downloading {} for {}: {}",
        asset_name, target_triple, download_url
    );

    // Download the asset
    let response = client.get(download_url).send().await?;
    let bytes = response.bytes().await?;

    // Extract to appropriate directory
    extract_dependency(repo, target_triple, asset_name, &bytes).await?;

    Ok(())
}

async fn extract_dependency(
    repo: &str,
    target_triple: &str,
    asset_name: &str,
    bytes: &[u8],
) -> Result<()> {
    use std::io::Cursor;

    let base_dir = github_prefix_dir();

    if repo == "pahkat" {
        // libpahkat_client comes with jniLibs structure already, extract directly
        let extract_dir = base_dir.join("pkg").join(repo);
        std::fs::create_dir_all(&extract_dir)?;

        if asset_name.ends_with(".tgz") || asset_name.ends_with(".tar.gz") {
            extract_tar_gz(bytes, &extract_dir)?;
        } else {
            return Err(anyhow::anyhow!(
                "Expected .tgz format for libpahkat_client, got: {}",
                asset_name
            ));
        }

        println!("Extracted {} to {}", asset_name, extract_dir.display());
    } else {
        // Other dependencies like divvunspell need architecture-specific structure
        let arch = match target_triple {
            "aarch64-linux-android" => "arm64-v8a",
            "armv7-linux-androideabi" => "armeabi-v7a",
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported target triple: {}",
                    target_triple
                ));
            }
        };

        let extract_dir = base_dir.join("pkg").join(repo).join("lib").join(arch);
        std::fs::create_dir_all(&extract_dir)?;

        // Handle different archive formats
        if asset_name.ends_with(".zip") {
            extract_zip_flattened(bytes, &extract_dir)?;
        } else if asset_name.ends_with(".tar.gz") || asset_name.ends_with(".tgz") {
            extract_tar_gz_flattened(bytes, &extract_dir)?;
        } else if asset_name.ends_with(".tar.xz") {
            return Err(anyhow::anyhow!("tar.xz not supported yet"));
        } else {
            return Err(anyhow::anyhow!(
                "Unsupported archive format: {}",
                asset_name
            ));
        }

        println!("Extracted {} to {}", asset_name, extract_dir.display());
    }

    Ok(())
}

fn extract_zip(bytes: &[u8], extract_dir: &PathBuf) -> Result<()> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = extract_dir.join(file.name());

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
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

fn extract_zip_flattened(bytes: &[u8], extract_dir: &PathBuf) -> Result<()> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let file_path = std::path::Path::new(file.name());

        // Skip directories
        if file.name().ends_with('/') {
            continue;
        }

        // Extract only .so files and flatten the path
        if let Some(filename) = file_path.file_name() {
            if let Some(filename_str) = filename.to_str() {
                if filename_str.ends_with(".so") {
                    let outpath = extract_dir.join(filename);
                    let mut outfile = std::fs::File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }
        }
    }
    Ok(())
}

fn extract_tar_gz_flattened(bytes: &[u8], extract_dir: &PathBuf) -> Result<()> {
    use flate2::read::GzDecoder;
    use std::io::Cursor;
    use tar::Archive;

    let cursor = Cursor::new(bytes);
    let gz = GzDecoder::new(cursor);
    let mut archive = Archive::new(gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        // Skip directories
        if entry.header().entry_type().is_dir() {
            continue;
        }

        // Extract only .so files and flatten the path
        if let Some(filename) = path.file_name() {
            if let Some(filename_str) = filename.to_str() {
                if filename_str.ends_with(".so") {
                    let outpath = extract_dir.join(filename);
                    entry.unpack(outpath)?;
                }
            }
        }
    }
    Ok(())
}
