// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use bollard::Docker;
use chrono::Utc;
use filetime::FileTime;
use fs_extra::dir;
use serde::Deserialize;
use slog::{info, Logger};
use std::{
    fs,
    path::{Path, PathBuf},
    vec,
};
use thiserror::Error;
use tokio::time::{Duration, Instant};
use url::{ParseError, Url};

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

        docker.stop_container(&self.node_container_name, None).await?;

        info!(self.log, "Tezedge node container stopped");

        docker.stop_container(&self.monitoring_container_name, None).await?;
        
        Ok(())
    }

    /// Starts the tezedge container
    pub async fn start(&self) -> Result<(), TezedgeNodeControllerError> {
        let docker = Docker::connect_with_socket_defaults()?;

        docker.start_container::<String>(&self.node_container_name, None).await?;

        info!(self.log, "Tezedge node container started");

        docker.start_container::<String>(&self.monitoring_container_name, None).await?;

        info!(self.log, "Tezedge node monitoring container started");
        // TODO: preform a health check with get_head

        Ok(())
    }

    /// Takes a snapshot of the tezedge node
    pub async fn take_snapshot(
        &mut self,
        snapshot_capacity: usize,
    ) -> Result<(), TezedgeNodeControllerError> {
        self.last_snapshot_timestamp = Some(Instant::now());
        let head_block_hash = self.get_head().await?.hash;

        // 1. stop the node container
        info!(self.log, "Stopping tezedge container (1/6)");
        self.stop().await?;

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

        // we start by giving the directory a "temporary" name so we can ignore it until the copy has finished
        let snapshot_name_temp = format!("{}_{}_{}-{}_{}.temp", "tezedge", self.network, date, time, head_block_hash);
        let snapshot_name = format!("{}_{}_{}-{}_{}", "tezedge", self.network, date, time, head_block_hash);

        info!(self.log, "Checking for rolling (2/6)");

        // identify and remove the oldest snapshot in the target dir, if we are over capacity
        let current_snapshots = dir::get_dir_content(&self.snapshots_target_directory)?
            .directories
            .iter()
            .map(|dir| self.snapshots_target_directory.join(dir))
            // we need the only the direct directories contained in the main directory, filter out all deeper sub directories
            .filter(|p| {
                p.components().count() == self.snapshots_target_directory.components().count() + 1
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

        // 2. copy out the database directories to a temp folder
        info!(self.log, "Extracting node databases (3/6)");

        let copy_options = dir::CopyOptions {
            content_only: true,
            ..Default::default()
        };

        let temp_destination = Path::new("/tmp/tezedge-snapshots-tmp");
        let snapshot_path =
            temp_destination.join(Path::new(&snapshot_name_temp));

        if !snapshot_path.exists() {
            dir::create_all(&snapshot_path, false)?;
        }

        let to_remove = vec![self.database_directory.join("context/index/lock")];
        fs_extra::remove_items(&to_remove)?;

        dir::copy(&self.database_directory, &snapshot_path, &copy_options)?;

        // 3. remove identity and log files
        info!(self.log, "Removing unnecessary files (log, identity) (4/6)");

        // collect all log files present in the snapshot
        let mut to_remove: Vec<PathBuf> = dir::get_dir_content(&snapshot_path)?
            .files
            .iter()
            .filter(|s| s.contains(".log"))
            .map(|s| snapshot_path.join(s))
            .collect();

        to_remove.push(snapshot_path.join("identity.json"));
        fs_extra::remove_items(&to_remove)?;

        // 5. move to the destination
        info!(self.log, "Moving snapshot to the target directory (5/6)");
        let copy_options = dir::CopyOptions::new();
        dir::move_dir(
            snapshot_path.clone(),
            &self.snapshots_target_directory,
            &copy_options,
        )?;

        // rename to the final name removing .temp indicating that the copy has been complete
        fs::rename(self.snapshots_target_directory.join(&snapshot_name_temp), self.snapshots_target_directory.join(&snapshot_name))?;

        // remove the tmp folder
        fs_extra::remove_items(&[snapshot_path])?;

        // 6. start the node container back up
        info!(self.log, "Starting back up the tezedge container (6/6)");
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
