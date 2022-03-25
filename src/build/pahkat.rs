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

    let store = create_prefix().await;
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

pub fn prefix_dir() -> PathBuf {
    let kbdgen_data = pathos::user::app_data_dir("kbdgen").unwrap();
    kbdgen_data.join("prefix")
}

async fn create_prefix() -> Arc<dyn PackageStore> {
    let prefix_path = prefix_dir();
    tracing::info!("Prefix: {}", prefix_path.display());

    tracing::info!("opening package store");
    let prefix = PrefixPackageStore::open_or_create(&prefix_path)
        .await
        .unwrap();
    tracing::info!("opened package store");
    let config = prefix.config();

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
    drop(prefix);

    // We can't just refresh repos because it locks up, reason unknown.
    let prefix = PrefixPackageStore::open(&prefix_path).await.unwrap();
    Arc::new(prefix)
}
