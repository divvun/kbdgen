use serde_json::Value;
use std::collections::HashMap;

pub async fn install_android_deps() {
    // get the info about the latest releases of divvunspell and
    println!("ANDROID DEPS");
    let res = download_latest_release("divvun", "divvunspell", None).await;
}

pub async fn download_latest_release(
    org: &str,
    repo: &str,
    filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://api.github.com/repos/{org}/{repo}/releases/latest");
    let client = reqwest::Client::new();
    let json: Value = client
        .get(&url)
        .header("User-Agent", "rust-client")
        .send()
        .await?
        .json()
        .await?;

    let asset = json["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find(|asset| {
                asset["name"]
                    .as_str()
                    .map(|name| name.to_lowercase().contains("aarch64-linux-android"))
                    .unwrap_or(false)
            })
        })
        .ok_or("No Android asset found")?;

    let download_url = asset["browser_download_url"].as_str().unwrap();

    println!("download_url: {:?}", download_url);
    Ok(())
}
