use std::{
    collections::HashMap,
    path::Path,
    process::{Command, Stdio},
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{
    build::{
        BuildStep,
        ios::{IosProjectExt, ensure_success, ios_target, keyboard_name, pbxproj::Pbxproj},
    },
    bundle::{KbdgenBundle, target::iOS},
};

const REPOSITORY: &str = "repo";

const FASTLANE_JOBS: usize = 12;

pub struct BuildXcarchive;

fn xcodebuild_archive(
    repository_path: &Path,
    archive_path: &Path,
    code_sign_id: &str,
    team_id: &str,
) -> Result<()> {
    tracing::info!("Archiving {}", archive_path.display());
    let mut xcodebuild = Command::new("xcodebuild")
        .current_dir(repository_path)
        .args(["archive", "-archivePath"])
        .arg(archive_path)
        .args([
            "-workspace",
            "GiellaKeyboard.xcworkspace",
            "-configuration",
            "Release",
            "-scheme",
            "HostingApp",
            "-destination",
            "generic/platform=iOS",
        ])
        .arg("-jobs")
        .arg(num_cpus::get().to_string())
        .arg(format!("CODE_SIGN_IDENTITY={code_sign_id}"))
        .arg(format!("DEVELOPMENT_TEAM={team_id}"))
        .stdout(Stdio::piped())
        .spawn()
        .context("`xcodebuild` could not be started; install Xcode")?;
    let log = xcodebuild
        .stdout
        .take()
        .expect("xcodebuild's stdout is piped");

    let beautified = Command::new("xcbeautify").stdin(log).status();
    let status = xcodebuild.wait().context("wait for `xcodebuild archive`")?;
    let beautified = beautified
        .context("`xcbeautify` could not be started; install it with `brew install xcbeautify`")?;
    if !status.success() {
        bail!("`xcodebuild archive` failed with {status}; its log is above");
    }
    if !beautified.success() {
        bail!("`xcbeautify` failed with {beautified}");
    }
    Ok(())
}

fn xcodebuild_export_ipa(
    repository_path: &Path,
    archive_path: &Path,
    ipa_path: &Path,
    plist_path: &Path,
) -> Result<()> {
    tracing::info!("Exporting {}", ipa_path.display());
    let status = Command::new("xcodebuild")
        .current_dir(repository_path)
        .args(["-exportArchive", "-archivePath"])
        .arg(archive_path)
        .arg("-exportPath")
        .arg(ipa_path)
        .arg("-exportOptionsPlist")
        .arg(plist_path)
        .status()
        .context("`xcodebuild` could not be started; install Xcode")?;
    if !status.success() {
        bail!("`xcodebuild -exportArchive` failed with {status}; its log is above");
    }
    Ok(())
}

// [spec:kbdgen:req:ios.build-chain+2]
#[async_trait(?Send)]
impl BuildStep for BuildXcarchive {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let target = ios_target(bundle)?;
        let team_id = target
            .team_id
            .as_deref()
            .context("targets/ios.yaml has no teamId")?;
        let code_sign_id = target
            .code_sign_id
            .as_deref()
            .context("targets/ios.yaml has no codeSignId")?;
        let repository_path = output_path.join(REPOSITORY);

        let archive_path = output_path.join(format!("{}.xcarchive", target.package_id));
        let ipa_path = output_path.join("ipa");
        let plist_path = repository_path.join("opts.plist");

        xcodebuild_archive(&repository_path, &archive_path, code_sign_id, team_id)?;
        xcodebuild_export_ipa(&repository_path, &archive_path, &ipa_path, &plist_path)
    }
}

pub fn fastlane_env(ios_target: &iOS) -> HashMap<&'static str, String> {
    [
        ("MATCH_GIT_URL", &ios_target.match_git_url),
        ("MATCH_PASSWORD", &ios_target.match_password),
        ("FASTLANE_USER", &ios_target.fastlane_user),
        ("PRODUCE_USERNAME", &ios_target.fastlane_user),
        ("FASTLANE_PASSWORD", &ios_target.fastlane_password),
        ("APP_STORE_KEY_JSON", &ios_target.app_store_key_json),
    ]
    .into_iter()
    .filter_map(|(name, value)| Some((name, value.clone()?)))
    .collect()
}

pub async fn fastlane(
    dir: &Path,
    env: &HashMap<&'static str, String>,
    args: &[&str],
) -> Result<()> {
    let command = format!("fastlane {}", args.join(" "));
    tracing::debug!("Running {command}");
    let output = tokio::process::Command::new("fastlane")
        .current_dir(dir)
        .envs(env)
        .args(args)
        .output()
        .await
        .context("`fastlane` could not be started; install it with `brew install fastlane`")?;
    tracing::trace!(
        "Output of {command}:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    ensure_success(&command, &output)
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
    repository_path: &Path,
    id: &str,
) -> Result<ProvisioningProfile> {
    let profile = format!("{id}.mobileprovision");
    let output = tokio::process::Command::new("security")
        .args(["cms", "-D", "-i", &profile])
        .current_dir(repository_path)
        .output()
        .await
        .context("`security` could not be started")?;
    ensure_success(&format!("security cms -D -i {profile}"), &output)?;
    plist::from_bytes(&output.stdout).with_context(|| format!("parse {profile}"))
}

// [spec:kbdgen:req:ios.build-chain+2]
async fn embed_profiles(
    bundle: &KbdgenBundle,
    target: &iOS,
    team_id: &str,
    repository_path: &Path,
) -> Result<()> {
    let pbxproj_path = repository_path.join("GiellaKeyboard.xcodeproj/project.pbxproj");
    let mut pbxproj = Pbxproj::from_path(&pbxproj_path)?;

    let mut plist = Plist {
        team_id: team_id.to_string(),
        method: "app-store".to_string(),
        provisioning_profiles: Default::default(),
    };

    for id in bundle.all_pkg_ids()? {
        let target_name = if id == target.package_id {
            "HostingApp"
        } else {
            keyboard_name(&id)
        };
        let profile = load_provisioning_profile(repository_path, &id).await?;
        tracing::debug!("Using profile {} ({}) for {id}", profile.name, profile.uuid);

        pbxproj.set_build_setting(target_name, "PROVISIONING_PROFILE", &profile.uuid)?;
        pbxproj.set_build_setting(target_name, "PROVISIONING_PROFILE_SPECIFIER", &profile.name)?;
        plist.provisioning_profiles.insert(id, profile.uuid);
    }

    tracing::debug!("Writing {}", pbxproj_path.display());
    std::fs::write(&pbxproj_path, pbxproj.to_pbxproj_string())
        .with_context(|| format!("write {}", pbxproj_path.display()))?;
    let opts_path = repository_path.join("opts.plist");
    tracing::debug!("Writing {}", opts_path.display());
    plist::to_file_xml(&opts_path, &plist).with_context(|| format!("write {}", opts_path.display()))
}

// [spec:kbdgen:req:ios.build-chain+2]
#[async_trait(?Send)]
impl BuildStep for FastlaneProvisioning {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let target = ios_target(bundle)?;
        let api_key_path = target
            .app_store_key_json
            .as_deref()
            .context("targets/ios.yaml has no appStoreKeyJson")?;
        let team_id = target
            .team_id
            .as_deref()
            .context("targets/ios.yaml has no teamId")?;
        let repository_path = output_path.join(REPOSITORY);
        let ids = bundle.all_pkg_ids()?;

        let env = fastlane_env(target);
        tracing::trace!(
            "Fastlane environment sets {}",
            env.keys().copied().collect::<Vec<_>>().join(", ")
        );

        tracing::info!(
            "Downloading signing certificates and provisioning profiles for {} package ids",
            ids.len()
        );
        tracing::debug!("Package ids: {}", ids.join(", "));

        let api_key_arg = format!("--api_key_path={api_key_path}");
        let owned = |args: &[&str]| args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        let match_runs = ids.iter().map(|id| {
            owned(&[
                "match",
                "appstore",
                &format!("--app_identifier={id}"),
                &api_key_arg,
            ])
        });
        let sigh_runs = ids.iter().map(|id| {
            owned(&[
                "sigh",
                "-a",
                id,
                "-b",
                team_id,
                "-z",
                "-q",
                &format!("{id}.mobileprovision"),
                &api_key_arg,
            ])
        });
        let runs: Vec<Vec<String>> = match_runs.chain(sigh_runs).collect();

        let jobs = Arc::new(Semaphore::new(FASTLANE_JOBS));
        let tasks: Vec<_> = runs
            .into_iter()
            .map(|args| {
                let jobs = jobs.clone();
                let repository_path = repository_path.clone();
                let env = env.clone();
                tokio::spawn(async move {
                    let _job = jobs
                        .acquire_owned()
                        .await
                        .expect("the semaphore is never closed");
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    fastlane(&repository_path, &env, &args).await
                })
            })
            .collect();
        for task in tasks {
            task.await.context("a fastlane task panicked")??;
        }

        tracing::info!("Downloaded all signing certificates and provisioning profiles");
        embed_profiles(bundle, target, team_id, &repository_path).await
    }
}
