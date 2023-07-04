use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    process::{Child, ExitStatus},
};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    build::{
        ios::{pbxproj::Pbxproj, IosProjectExt},
        BuildStep,
    },
    bundle::{target::iOS, KbdgenBundle},
};

pub struct BuildXcarchive;

fn xcodebuild_archive(
    deps_path: &Path,
    archive_path: &Path,
    code_sign_id: &str,
    team_id: &str,
) -> io::Result<ExitStatus> {
    std::process::Command::new("xcodebuild")
        .current_dir(deps_path)
        .args(["archive", "-archivePath"])
        .arg(archive_path)
        .args([
            "-workspace",
            "GiellaKeyboard.xcworkspace",
            "-configuration",
            "Release",
            "-scheme",
            "HostingApp",
        ])
        .arg("-jobs")
        .arg(num_cpus::get().to_string())
        // .env("CODE_SIGN_IDENTITY", code_sign_id)
        .arg(format!("CODE_SIGN_IDENTITY={}", code_sign_id))
        .arg(format!("DEVELOPMENT_TEAM={}", team_id))
        .status()
}

fn xcodebuild_export_ipa(
    deps_path: &Path,
    archive_path: &Path,
    ipa_path: &Path,
    plist_path: &Path,
) -> io::Result<ExitStatus> {
    std::process::Command::new("xcodebuild")
        .current_dir(deps_path)
        .args(["-exportArchive", "-archivePath"])
        .arg(archive_path)
        .arg("-exportPath")
        .arg(ipa_path)
        .arg("-exportOptionsPlist")
        .arg(plist_path)
        .status()
}

#[async_trait(?Send)]
impl BuildStep for BuildXcarchive {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> anyhow::Result<()> {
        let target = bundle.targets.ios.as_ref().unwrap().clone();
        let team_id = target.team_id.as_ref().unwrap().to_string();
        let code_sign_id = target.code_sign_id.as_ref().unwrap().to_string();
        let deps_path = output_path.join("repo");

        let archive_path = output_path.join(format!("{}.xcarchive", target.package_id));
        let ipa_path = output_path.join("ipa");
        let plist_path = output_path.join("repo").join("opts.plist");

        xcodebuild_archive(&deps_path, &archive_path, &code_sign_id, &team_id)?;
        xcodebuild_export_ipa(&deps_path, &archive_path, &ipa_path, &plist_path)?;

        Ok(())
    }
}

pub fn fastlane_env(ios_target: &iOS) -> HashMap<&'static str, String> {
    let mut o = HashMap::new();

    if let Some(value) = ios_target.match_git_url.as_deref() {
        o.insert("MATCH_GIT_URL", value.to_string());
    }
    if let Some(value) = ios_target.match_password.as_deref() {
        o.insert("MATCH_PASSWORD", value.to_string());
    }
    if let Some(value) = ios_target.fastlane_user.as_deref() {
        o.insert("FASTLANE_USER", value.to_string());
        o.insert("PRODUCE_USERNAME", value.to_string());
    }
    if let Some(value) = ios_target.fastlane_password.as_deref() {
        o.insert("FASTLANE_PASSWORD", value.to_string());
    }
    if let Some(value) = ios_target.app_store_key_json.as_deref() {
        o.insert("APP_STORE_KEY_JSON", value.to_string());
    }

    o
}

pub struct FastlaneProvisioning;

#[derive(Debug, Clone, Serialize)]
struct Plist {
    #[serde(rename = "teamID")]
    team_id: String,
    method: String,
    #[serde(rename = "provisioningProfiles")]
    provisioning_profiles: IndexMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProvisioningProfile {
    #[serde(rename = "UUID")]
    uuid: String,
    #[serde(rename = "Name")]
    name: String,
}

async fn load_provisioning_profile(
    deps_path: &Path,
    id: &str,
) -> anyhow::Result<ProvisioningProfile> {
    let output = tokio::process::Command::new("security")
        .args(["cms", "-D", "-i"])
        .arg(format!("{id}.mobileprovision"))
        .current_dir(deps_path)
        .output()
        .await?;
    Ok(plist::from_bytes(&output.stdout)?)
}

async fn embed_profiles(
    bundle: &KbdgenBundle,
    deps_path: &Path,
    output_path: &Path,
) -> anyhow::Result<()> {
    let target = bundle.targets.ios.as_ref().unwrap().clone();
    let team_id = target.team_id.as_ref().unwrap().to_string();

    let repository_path = output_path.join("repo");

    let xcodeproj_path = repository_path.join("GiellaKeyboard.xcodeproj");
    let pbxproj_path = xcodeproj_path.join("project.pbxproj");
    let mut pbxproj = Pbxproj::from_path(&pbxproj_path);

    let mut plist = Plist {
        team_id,
        method: "app-store".to_string(),
        provisioning_profiles: Default::default(),
    };

    for id in bundle.all_pkg_ids() {
        let name = if id == target.package_id {
            "HostingApp"
        } else {
            id.split(".").last().unwrap()
        };
        let profile = load_provisioning_profile(deps_path, &id).await?;

        pbxproj.set_build_target_setting(name, "PROVISIONING_PROFILE", &profile.uuid);
        pbxproj.set_build_target_setting(name, "PROVISIONING_PROFILE_SPECIFIER", &profile.name);
        plist
            .provisioning_profiles
            .insert(id.to_string(), profile.uuid);
    }

    std::fs::write(pbxproj_path, pbxproj.to_pbxproj_string())?;
    plist::to_file_xml(deps_path.join("opts.plist"), &plist)?;

    Ok(())
}

#[async_trait(?Send)]
impl BuildStep for FastlaneProvisioning {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> anyhow::Result<()> {
        let target = bundle.targets.ios.as_ref().unwrap().clone();
        let app_store_key_json_path = target.app_store_key_json.as_deref().unwrap().to_string();
        let team_id = target.team_id.as_ref().unwrap().to_string();
        let deps_path = output_path.join("repo");

        tracing::info!(
            "Downloading signing certificates and provisioning profiles (this may take a while)..."
        );

        let env = fastlane_env(&target);
        tracing::trace!(env = ?env, "fastlane env");

        let mut futures = vec![];
        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(12));

        tracing::info!("bundle ids: {:?}", bundle.all_pkg_ids());

        bundle.all_pkg_ids().into_iter().for_each(|id| {
            // let team_id = team_id.to_string();
            let app_store_key_json_path = app_store_key_json_path.to_string();
            let deps_path = deps_path.clone();
            let env = fastlane_env(&target);
            let sem = sem.clone();

            futures.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await.unwrap();

                tokio::process::Command::new("fastlane")
                    .current_dir(&deps_path)
                    .envs(&env)
                    .args(["match", "appstore"])
                    .arg(format!("--app_identifier={}", &id))
                    .arg(format!("--api_key_path={}", app_store_key_json_path))
                    .output()
                    .await
                    .map(|x| (id, x, true))
            }))
        });

        bundle.all_pkg_ids().into_iter().for_each(|id| {
            let team_id = team_id.to_string();
            let app_store_key_json_path = app_store_key_json_path.to_string();
            let deps_path = deps_path.clone();
            let env = fastlane_env(&target);
            let sem = sem.clone();

            futures.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await.unwrap();

                tokio::process::Command::new("fastlane")
                    .current_dir(&deps_path)
                    .envs(&env)
                    .args(["sigh", "-a"])
                    .arg(&id)
                    .arg("-b")
                    .arg(team_id)
                    .args(["-z", "-q"])
                    .arg(format!("{}.mobileprovision", &id))
                    .arg(format!("--api_key_path={}", app_store_key_json_path))
                    .output()
                    .await
                    .map(|x| (id, x, false))
            }))
        });

        for fut in futures {
            let fut = fut.await;
            match fut {
                Ok(Ok((id, output, is_signing))) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output.status.success() {
                        tracing::trace!(exit_code = output.status.code(), stdout = %stdout, stderr = %stderr);
                    } else {
                        tracing::error!(exit_code = output.status.code(), stdout = %stdout, stderr = %stderr);
                    }

                    if !output.status.success() {
                        anyhow::bail!("Failed to download profile '{}'.", id);
                    }

                    if is_signing {
                        tracing::debug!("Downloaded signing cert for '{}'.", id);
                    } else {
                        tracing::debug!("Downloaded profile for '{}'.", id);
                    }
                }
                Ok(Err(err)) => return Err(err.into()),
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        tracing::info!("Downloaded all signing certs and provisioning profiles.");
        embed_profiles(bundle, &deps_path, output_path).await?;

        Ok(())
    }
}
