use std::{convert::TryInto, path::PathBuf, sync::Arc};

use futures::stream::StreamExt;

use pahkat_client::transaction::{PackageAction, PackageTransaction};
use pahkat_client::types::{repo::RepoUrl, PackageKey};
use pahkat_client::{
    config::RepoRecord, package_store::prefix::PrefixPackageStore,
    types::package_key::PackageKeyParams, InstallTarget, PackageStore,
};

pub async fn install_msklc() {
    tracing::info!("Updating 'msklc'...");

    let store = create_prefix("windows").await;
    tracing::debug!("Got a prefix");

    let repo_url: RepoUrl = "https://pahkat.uit.no/devtools/".parse().unwrap();

    let pkg_key = PackageKey::new_unchecked(
        repo_url.clone(),
        "msklc".to_string(),
        Some(PackageKeyParams {
            channel: Some("nightly".to_string()),
            ..Default::default()
        }),
    );

    let actions = vec![PackageAction::install(pkg_key, InstallTarget::System)];

    tracing::debug!("Creating package transaction");
    let tx = PackageTransaction::new(Arc::clone(&store as _), actions).unwrap();

    tracing::debug!("Beginning downloads");
    for record in tx.actions().iter() {
        let action = &record.action;
        let mut download = store.download(&action.id);

        use pahkat_client::package_store::DownloadEvent;

        while let Some(event) = download.next().await {
            match event {
                DownloadEvent::Error(e) => {
                    tracing::error!("{:?}", &e);
                    std::process::exit(1);
                }
                event => {
                    tracing::debug!("{:?}", &event);
                }
            };
        }
    }

    let (_cancel, mut stream) = tx.process();

    while let Some(value) = stream.next().await {
        println!("{:?}", value);
    }
}

pub async fn install_android_deps() {
    tracing::info!("Updating 'libdivvunspell' and 'libpahkat_client'...");

    let store = create_prefix("android").await;
    tracing::debug!("Got a prefix");

    let repo_url: RepoUrl = "https://pahkat.uit.no/devtools/".parse().unwrap();

    let divvunspell_pkg_key = PackageKey::new_unchecked(
        repo_url.clone(),
        "libdivvunspell".to_string(),
        Some(PackageKeyParams {
            channel: Some("nightly".to_string()),
            platform: Some("android".to_string()),
            ..Default::default()
        }),
    );

    let pahkat_client_pkg_key = PackageKey::new_unchecked(
        repo_url.clone(),
        "libpahkat_client".to_string(),
        Some(PackageKeyParams {
            channel: Some("nightly".to_string()),
            platform: Some("android".to_string()),
            ..Default::default()
        }),
    );

    let actions = vec![
        PackageAction::install(divvunspell_pkg_key, InstallTarget::System),
        PackageAction::install(pahkat_client_pkg_key, InstallTarget::System),
    ];

    tracing::debug!("Creating package transaction");
    let tx = PackageTransaction::new(Arc::clone(&store as _), actions).unwrap();

    tracing::debug!("Beginning downloads");
    for record in tx.actions().iter() {
        let action = &record.action;
        let mut download = store.download(&action.id);

        use pahkat_client::package_store::DownloadEvent;

        while let Some(event) = download.next().await {
            match event {
                DownloadEvent::Error(e) => {
                    tracing::error!("{:?}", &e);
                    std::process::exit(1);
                }
                event => {
                    tracing::debug!("{:?}", &event);
                }
            };
        }
    }

    let (_cancel, mut stream) = tx.process();

    while let Some(value) = stream.next().await {
        println!("{:?}", value);
    }
}

pub fn prefix_dir(platform: &str) -> PathBuf {
    let kbdgen_data = pathos::user::app_data_dir("kbdgen").unwrap();
    kbdgen_data.join("prefix").join(platform)
}

async fn create_prefix(platform: &str) -> Arc<dyn PackageStore> {
    let prefix_path = prefix_dir(platform);
    tracing::info!("Prefix: {}", prefix_path.display());
    let prefix = PrefixPackageStore::open_or_create(&prefix_path)
        .await
        .unwrap();
    let config = prefix.config();

    {
        let mut config = config.write().unwrap();
        let settings = config.settings_mut();
        settings
            .set_cache_dir(
                pathos::user::app_cache_dir("kbdgen")
                    .unwrap()
                    .try_into()
                    .unwrap(),
            )
            .unwrap();
        settings
            .set_tmp_dir(
                pathos::user::app_temporary_dir("kbdgen")
                    .unwrap()
                    .try_into()
                    .unwrap(),
            )
            .unwrap();

        let repos = config.repos_mut();
        repos
            .insert(
                "https://pahkat.uit.no/devtools/".parse().unwrap(),
                RepoRecord {
                    channel: Some("nightly".into()),
                },
            )
            .unwrap();
    }

    tracing::debug!("Refreshing repos...");
    prefix.refresh_repos().await.unwrap();

    tracing::debug!("Done refreshing");
    Arc::new(prefix)
}
