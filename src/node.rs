// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use bollard::{
    container::{Config, CreateContainerOptions, ListContainersOptions},
    models::{HostConfig, Mount, MountTypeEnum},
    Docker,
};
use chrono::Utc;
use filetime::FileTime;
use fs_extra::dir;
use serde::Deserialize;
use slog::{info, Logger};
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    vec,
};
use thiserror::Error;
use tokio::time::{Duration, Instant};
use url::{ParseError, Url};

use crate::configuration::SnapshotType;

#[derive(Clone, Debug, Deserialize)]
pub struct TezosBlockHeader {
    hash: String,
}
pub struct TezedgeNodeController {
    url: Url,
    node_container_name: String,
    monitoring_container_name: String,
    network: String,
    database_directory: PathBuf,
    last_snapshot_timestamp: Option<Instant>,
    snapshots_target_directory: PathBuf,
    log: Logger,
}

#[derive(Debug, Error)]
pub enum TezedgeNodeControllerError {
    #[error("The defined tezedge node is unreachable")]
    NodeUnreachable,
    #[error("Failed to parse url: {0}")]
    MalformedUrl(#[from] ParseError),
    #[error("Request to the node failed: {0}")]
    FailedRequest(#[from] reqwest::Error),
    #[error("Docker operation failed: {0}")]
    DockerError(#[from] bollard::errors::Error),
    #[error("Filesystem operation failed: {0}")]
    FilesystemError(#[from] fs_extra::error::Error),
    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),
}

impl TezedgeNodeController {
    pub fn new(
        url: Url,
        node_container_name: String,
        monitoring_container_name: String,
        network: String,
        database_directory: PathBuf,
        snapshots_target_directory: PathBuf,
        log: Logger,
    ) -> Self {
        Self {
            url,
            node_container_name,
            monitoring_container_name,
            network,
            database_directory,
            snapshots_target_directory,
            last_snapshot_timestamp: None,
            log,
        }
    }

    /// Gets the head header from the node
    pub async fn get_head(&self) -> Result<TezosBlockHeader, TezedgeNodeControllerError> {
        let header_url = self.url.join("chains/main/blocks/head/header")?;
        let head_header = reqwest::get(header_url).await?.json().await?;

        Ok(head_header)
    }

    /// Stops the tezedge container
    pub async fn stop(&self) -> Result<(), TezedgeNodeControllerError> {
        let docker = Docker::connect_with_socket_defaults()?;

        docker
            .stop_container(&self.node_container_name, None)
            .await?;

        info!(self.log, "Tezedge node container stopped");

        docker
            .stop_container(&self.monitoring_container_name, None)
            .await?;

        Ok(())
    }

    /// Starts the tezedge container
    pub async fn start(&self) -> Result<(), TezedgeNodeControllerError> {
        let docker = Docker::connect_with_socket_defaults()?;

        docker
            .start_container::<String>(&self.node_container_name, None)
            .await?;

        info!(self.log, "Tezedge node container started");

        docker
            .start_container::<String>(&self.monitoring_container_name, None)
            .await?;

        info!(self.log, "Tezedge node monitoring container started");
        // TODO: preform a health check with get_head

        Ok(())
    }

    async fn take_archive_snapshot(
        &mut self,
        snapshot_capacity: usize,
        snapshot_name: &str,
    ) -> Result<PathBuf, TezedgeNodeControllerError> {
        // we start by giving the directory a "temporary" name so we can ignore it until the copy has finished
        let snapshot_name_temp = format!("{}.temp", snapshot_name);

        let archive_snapshot_name = format!("{}_archive", snapshot_name);

        let archive_snapshots_target_directory = self.snapshots_target_directory.join("archive");

        if !archive_snapshots_target_directory.exists() {
            dir::create_all(&archive_snapshots_target_directory, false)?;
        }

        info!(self.log, "[Archive] Checking for rolling older snapshots (1/5)");

        // identify and remove the oldest snapshot in the target dir, if we are over capacity
        self.check_rolling(&archive_snapshots_target_directory, snapshot_capacity)?;

        // 2. copy out the database directories to a temp folder
        info!(self.log, "[Archive] Extracting node databases (2/5)");

        let copy_options = dir::CopyOptions {
            content_only: true,
            ..Default::default()
        };

        let temp_destination = Path::new("/tmp/tezedge-snapshots-tmp");
        // let temp_destination = self.database_directory.join("temp_destination");
        let snapshot_path = temp_destination.join(Path::new(&snapshot_name_temp));

        if !snapshot_path.exists() {
            dir::create_all(&snapshot_path, false)?;
        }

        let to_remove = vec![self.database_directory.join("context/index/lock")];
        fs_extra::remove_items(&to_remove)?;

        dir::copy(&self.database_directory, &snapshot_path, &copy_options)?;

        // 3. remove identity and log files
        info!(self.log, "[Archive] Removing unnecessary files (log, identity) (3/5)");

        // collect all log files present in the snapshot
        let mut to_remove: Vec<PathBuf> = dir::get_dir_content(&snapshot_path)?
            .files
            .iter()
            .filter(|s| s.contains(".log"))
            .map(|s| snapshot_path.join(s))
            .collect();

        to_remove.push(snapshot_path.join("identity.json"));
        fs_extra::remove_items(&to_remove)?;

        // 4. move to the destination
        info!(self.log, "[Archive] Moving snapshot to the target directory (4/5)");
        let copy_options = dir::CopyOptions::new();
        dir::move_dir(
            snapshot_path.clone(),
            &archive_snapshots_target_directory,
            &copy_options,
        )?;

        // . move to the destination
        info!(self.log, "[Archive] Removing .temp from the snapshot directory (5/5)");
        // rename to the final name removing .temp indicating that the copy has been complete
        fs::rename(
            archive_snapshots_target_directory.join(&snapshot_name_temp),
            archive_snapshots_target_directory.join(&archive_snapshot_name),
        )?;

        // remove the tmp folder
        fs_extra::remove_items(&[snapshot_path])?;

        Ok(archive_snapshots_target_directory.join(&archive_snapshot_name))
    }

    async fn take_full_snapshot(
        &self,
        from_dir: &str,
        snapshot_name: &str,
        snapshot_capacity: usize,
    ) -> Result<(), TezedgeNodeControllerError> {
        let docker = Docker::connect_with_socket_defaults()?;

        if self.database_directory.join("context/index/lock").exists() {
            let to_remove = vec![self.database_directory.join("context/index/lock")];
            fs_extra::remove_items(&to_remove)?;
        }

        let image = "tezedge/tezedge:latest";
        let cont_name = format!("tezedge-snapshots-full-{}", self.network);
        let snapshot_name = format!("{}_full", snapshot_name);
        let snapshot_name_temp = format!("{}.temp", &snapshot_name);

        let full_snapshots_target_directory = self.snapshots_target_directory.join("full");

        if !full_snapshots_target_directory.exists() {
            dir::create_all(&full_snapshots_target_directory, false)?;
        }

        // check for rolling
        info!(self.log, "[Full] Checking for rolling older snapshots (1/6)");
        self.check_rolling(&full_snapshots_target_directory, snapshot_capacity)?;

        let snapshot_path = full_snapshots_target_directory.join(&snapshot_name_temp);
        if !snapshot_path.exists() {
            dir::create_all(&snapshot_path, false)?;
        }
        let snapshot_path_string = snapshot_path.to_string_lossy().to_string();

        let entrypoint = vec![
            "/light-node",
            "--config-file=/tezedge.config",
            "--p2p-port=1234",
            "--rpc-port=1234",
            "--init-sapling-spend-params-file=/sapling-spend.params",
            "--init-sapling-output-params-file=/sapling-output.params",
            "--network",
            &self.network,
            "--bootstrap-db-path=bootstrap_db",
            "--tezos-data-dir",
            from_dir,
            "snapshot",
            "--target-path",
            &snapshot_path_string,
        ];

        info!(self.log, "[Full] Creating full snapshotting tezedge container (2/6)");
        let snapshot_host_path = env::var("TEZEDGE_SNAPSHOTS_VOLUME_PATH").unwrap_or_else(|_| {
            self.snapshots_target_directory
                .to_string_lossy()
                .to_string()
        });
        let tezedge_host_path = env::var("TEZEDGE_VOLUME_PATH").unwrap_or_else(|_| {
            self.snapshots_target_directory
                .to_string_lossy()
                .to_string()
        });
        let host_config = HostConfig {
            mounts: Some(vec![Mount {
                target: Some(
                    self.snapshots_target_directory
                        .to_string_lossy()
                        .to_string(),
                ),
                source: Some(snapshot_host_path.clone()),
                typ: Some(MountTypeEnum::BIND),
                ..Default::default()
            },
            Mount {
                target: Some(
                    self.database_directory
                        .to_string_lossy()
                        .to_string(),
                ),
                source: Some(tezedge_host_path.clone()),
                typ: Some(MountTypeEnum::BIND),
                ..Default::default()
            }
            ]),
            ..Default::default()
        };

        let config = Config {
            image: Some(image),
            host_config: Some(host_config),
            entrypoint: Some(entrypoint),
            ..Default::default()
        };

        let opts = CreateContainerOptions { name: cont_name.clone() };

        docker
            .create_container::<String, &str>(Some(opts), config)
            .await?;

        info!(self.log, "[Full] Starting full snapshotting tezedge container (3/6)");
        docker.start_container::<String>(&cont_name, None).await?;

        while let Ok(true) = Self::is_running(&cont_name).await {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        info!(self.log, "[Full] Full Snapshotting tezedge container finished (4/6)");

        // rename to the final name removing .temp indicating that the copy has been complete
        info!(self.log, "[Full] Removing .temp from the snapshot directory (5/6)");
        fs::rename(
            full_snapshots_target_directory.join(&snapshot_name_temp),
            full_snapshots_target_directory.join(&snapshot_name),
        )?;

        info!(self.log, "[Full] Removing Full Snapshotting tezedge container (6/6)");
        docker.remove_container(&cont_name, None).await?;

        Ok(())
    }

    async fn is_running(container_name: &str) -> Result<bool, TezedgeNodeControllerError> {
        let docker = Docker::connect_with_socket_defaults()?;

        let mut filter = HashMap::new();
        filter.insert(
            String::from("name"),
            vec![String::from(container_name)],
        );
        filter.insert(
            String::from("status"),
            vec![String::from("running")],
        );
        let containers = &docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: filter,
                ..Default::default()
            }))
            .await?;

        if containers.is_empty() {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn check_rolling(&self, snapshot_dir: &Path, snapshot_capacity: usize) -> Result<(), TezedgeNodeControllerError> {
        // identify and remove the oldest snapshot in the target dir, if we are over capacity
        let current_snapshots = dir::get_dir_content(&snapshot_dir)?
            .directories
            .iter()
            .map(|dir| snapshot_dir.join(dir))
            // we need the only the direct directories contained in the main directory, filter out all deeper sub directories
            .filter(|p| {
                p.components().count() == snapshot_dir.components().count() + 1
            })
            .collect::<Vec<PathBuf>>();

        // collect all last_modified times
        let mut dir_times: Vec<(PathBuf, FileTime)> = vec![];
        for snapshot_path in current_snapshots {
            let meta = fs::metadata(&snapshot_path)?;
            let last_modified = FileTime::from_last_modification_time(&meta);
            dir_times.push((snapshot_path, last_modified));
        }

        // sort by times
        dir_times.sort_by(|a, b| a.1.cmp(&b.1));

        // remove the oldest file if over capacity
        if dir_times.len() >= snapshot_capacity {
            info!(self.log, "Rolling snapshots - Removing oldest snapshot");
            fs_extra::remove_items(&[dir_times[0].0.clone()])?;
        }
        Ok(())
    }

    /// Takes a snapshot of the tezedge node
    pub async fn take_snapshot(
        &mut self,
        snapshot_capacity: usize,
        snapshot_type: &SnapshotType,
    ) -> Result<(), TezedgeNodeControllerError> {
        self.last_snapshot_timestamp = Some(Instant::now());
        let head_block_hash = self.get_head().await?.hash;

        // get the time for the snapshot title
        let now = Utc::now().naive_utc();
        let date = now.date().to_string().replace('-', "");
        let time: String = now
            .time()
            .to_string()
            .replace(':', "")
            .split('.')
            .take(1)
            .collect();

        let snapshot_name = format!(
            "{}_{}_{}-{}_{}",
            "tezedge", self.network, date, time, head_block_hash
        );

        // 1. stop the node container
        info!(self.log, "Stopping tezedge container");
        self.stop().await?;

        match snapshot_type {
            SnapshotType::Archive => {
                self.take_archive_snapshot(snapshot_capacity, &snapshot_name).await?;
            },
            SnapshotType::Full => {
                self.take_full_snapshot(
                    &self.database_directory
                        .to_string_lossy()
                        .to_string(),
                    &snapshot_name,
                    snapshot_capacity,
                )
                .await?;
            },
            SnapshotType::All => {
                let archive_snapshot_path = self.take_archive_snapshot(snapshot_capacity, &snapshot_name).await?;
                self.take_full_snapshot(
                        &archive_snapshot_path
                        .to_string_lossy()
                        .to_string(),
                    &snapshot_name,
                    snapshot_capacity,
                )
                .await?;
            },
        }

        // 6. start the node container back up
        info!(self.log, "Starting back up the tezedge container");
        self.start().await?;

        Ok(())
    }

    pub async fn can_snapshot(&self, snapshot_frequency: u64) -> bool {
        match self.get_head().await {
            Ok(_) => {
                if let Some(instant) = self.last_snapshot_timestamp {
                    instant.elapsed() >= Duration::from_secs(snapshot_frequency)
                } else {
                    true
                }
            }
            Err(_) => {
                // if the node does not respond to the rpc, do not snapshot
                // this catches a corner-case where, the node is started with a cleaned up DB
                // and is not yet ready for the first snapshot
                false
            }
        }
    }
}
