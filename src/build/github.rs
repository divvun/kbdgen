use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub async fn install_android_deps(jni_libs_path: &Path) -> Result<()> {
    println!("Installing Android dependencies from GitHub releases...");

    // Download divvunspell .so files directly to final destination
    for (target_triple, arch) in get_divvunspell_architectures() {
        let arch_dir = jni_libs_path.join(arch);
        std::fs::create_dir_all(&arch_dir)?;
        let so_path = arch_dir.join("libdivvunspell.so");
        download_so_file("divvun", "divvunspell", target_triple, &so_path).await?;
    }

    // Download pahkat jniLibs structure directly to final destination
    download_jnilibs_to_path("divvun", "pahkat", jni_libs_path).await?;

    Ok(())
}

pub fn get_divvunspell_architectures() -> &'static [(&'static str, &'static str)] {
    &[
        ("aarch64-linux-android", "arm64-v8a"),
        ("armv7-linux-androideabi", "armeabi-v7a"),
    ]
}

async fn download_so_file(
    org: &str,
    repo: &str,
    target_triple: &str,
    output_path: &PathBuf,
) -> Result<()> {
    let (asset_name, bytes) = download_asset(org, repo, target_triple).await?;

    // Extract .so file directly to output path
    extract_so_file(&bytes, &asset_name, output_path)?;

    println!("Saved {} to {}", asset_name, output_path.display());
    Ok(())
}

async fn download_jnilibs_to_path(org: &str, repo: &str, jni_libs_path: &Path) -> Result<()> {
    let (asset_name, bytes) = download_asset(org, repo, "android").await?;

    // Use temporary directory for extraction
    let temp_dir = tempfile::tempdir()?;
    extract_tar_gz(&bytes, temp_dir.path())?;

    // Copy jniLibs contents to final destination
    let extracted_jnilibs = temp_dir.path().join("jniLibs");
    if extracted_jnilibs.exists() {
        copy_dir_contents(&extracted_jnilibs, jni_libs_path)?;
    }

    println!(
        "Downloaded and installed {} to {}",
        asset_name,
        jni_libs_path.display()
    );
    Ok(())
}

fn copy_dir_contents(src_dir: &Path, dst_dir: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst_dir.join(entry.file_name());

        if src_path.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
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
        Err(anyhow::anyhow!(
            "Unsupported archive format: {}",
            asset_name
        ))
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

fn extract_tar_gz(bytes: &[u8], extract_dir: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use std::io::Cursor;
    use tar::Archive;

    let cursor = Cursor::new(bytes);
    let gz = GzDecoder::new(cursor);
    let mut archive = Archive::new(gz);
    archive.unpack(extract_dir)?;
    Ok(())
}
