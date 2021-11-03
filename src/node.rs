// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use chrono::Timelike;
use chrono::Utc;
use fs_extra::dir;
use serde::Deserialize;
use shiplift::Docker;
use std::collections::HashMap;
use std::{
    fmt::format,
    path::{Path, PathBuf},
    vec,
    fs,
};
use thiserror::Error;
use tokio::time::{Duration, Instant};
use url::{ParseError, Url};
use filetime::FileTime;

#[derive(Clone, Debug, Deserialize)]
pub struct TezosBlockHeader {
    protocol: String,
    chain_id: String,
    hash: String,
    level: i32,
    proto: i32, // TODO: verify
    predecessor: String,
    timestamp: String,   // TODO: rc-3999 format? Verify
    validation_pass: u8, // TODO: verify
    operations_hash: String,
    fitness: Vec<String>,
    context: String,
    priority: u64, //TODO: verify
    proof_of_work_nonce: String,
    signature: String,

    // introduced in newer protocols
    liquidity_baking_escape_vote: Option<bool>,
}
pub struct TezedgeNode {
    url: Url,
    container_name: String,
    database_directory: PathBuf,
    last_snapshot_timestamp: Option<Instant>,
    snapshots_target_directory: PathBuf,
}

// pub struct TezedgeSnapshotter {
//     snapshots_target_directory: PathBuf,
//     snapshot_capacity: u64,

//     snapshots: Vec<>,
// }

// pub struct Snapshot {
//     network: String,
//     timestamp:
// }

// impl TezedgeSnapshotter {
//     pub fn load_snapshots
// }

#[derive(Debug, Error)]
pub enum TezedgeNodeError {
    #[error("The defined tezedge node is unreachable")]
    NodeUnreachable,
    #[error("Failed to parse url: {0}")]
    MalformedUrl(#[from] ParseError),
    #[error("Request to the node failed: {0}")]
    FailedRequest(#[from] reqwest::Error),
    #[error("Docker operation failed: {0}")]
    DockerError(#[from] shiplift::Error),
    #[error("Filesystem operation failed: {0}")]
    FilesystemError(#[from] fs_extra::error::Error),
    // TODO: remove
    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),
}

impl TezedgeNode {
    pub fn new(
        url: Url,
        container_name: String,
        database_directory: PathBuf,
        snapshots_target_directory: PathBuf,
    ) -> Self {
        Self {
            url,
            container_name,
            database_directory,
            snapshots_target_directory,
            last_snapshot_timestamp: None,
        }
    }

    /// Gets the head header from the node
    pub async fn get_head(&self) -> Result<TezosBlockHeader, TezedgeNodeError> {
        let header_url = self.url.join("chains/main/blocks/head/header")?;
        let head_header = reqwest::get(header_url).await?.json().await?;

        Ok(head_header)
    }

    /// Stops the tezedge container
    pub async fn stop(&self) -> Result<(), TezedgeNodeError> {
        let docker = Docker::new();

        docker
            .containers()
            .get(&self.container_name)
            .stop(Some(Duration::from_secs(10)))
            .await?;

        Ok(())
    }

    /// Starts the tezedge container
    pub async fn start(&self) -> Result<(), TezedgeNodeError> {
        let docker = Docker::new();

        docker
            .containers()
            .get(&self.container_name)
            .start()
            .await?;

        // TODO: preform a health check with get_head

        Ok(())
    }

    /// Takes a snapshot of the tezedge node
    pub async fn take_snapshot(
        &mut self, /*snapshot_block_header: TezosBlockHeader*/
    ) -> Result<(), TezedgeNodeError> {
        self.last_snapshot_timestamp = Some(Instant::now());
        // 1. stop the node container
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

        // 2. copy out the database directories to a temp folder
        let copy_options = dir::CopyOptions {
            content_only: true,
            ..Default::default()
        };

        let temp_destination = Path::new("/tmp/tezedge-snapshots-tmp");
        let snapshot_path =
            temp_destination.join(Path::new(&format!("{}-{}-{}", "tezedge", date, time)));

        if !snapshot_path.exists() {
            // TODO: do this with fs_extra
            std::fs::create_dir_all(&snapshot_path)?;
        }

        let to_remove = vec![self.database_directory.join("context/index/lock")];
        fs_extra::remove_items(&to_remove)?;

        dir::copy(&self.database_directory, &snapshot_path, &copy_options)?;

        // 3. remove identity, log files and lock files

        // collect all log files present in the snapshot
        let mut to_remove: Vec<PathBuf> = dir::get_dir_content(&snapshot_path)?
            .files
            .iter()
            .filter(|s| s.contains(".log"))
            .map(|s| snapshot_path.join(s))
            .collect();

        to_remove.push(snapshot_path.join("identity.json"));
        fs_extra::remove_items(&to_remove)?;

        // 4. zip/tar it?
        // TODO

        // identify and remove the oldest snapshot in the target dir, if we are over capacity
        let current_snapshots = dir::get_dir_content(&self.snapshots_target_directory)?
            .directories
            .iter()
            .map(|dir| self.snapshots_target_directory.join(dir))
            // we need the only the direct directories contained in the main directory, filter out all deeper sub directories 
            .filter(|p| p.components().count() == self.snapshots_target_directory.components().count() + 1) 
            .collect::<Vec<PathBuf>>();

        let mut dir_times: Vec<(PathBuf, FileTime)> = vec![];
        for snapshot_path in current_snapshots {
            let meta = fs::metadata(&snapshot_path)?;
            let last_modified = FileTime::from_last_modification_time(&meta);
            dir_times.push((snapshot_path, last_modified));
        }

        // sort by times
        dir_times.sort_by(|a, b| {
            a.1.cmp(&b.1)
        });

        println!("{:?}", dir_times);

        // remove the oldest file if over capacity
        // TODO: capacity from config
        if dir_times.len() >= 7 {
            fs_extra::remove_items(&[dir_times[0].0.clone()])?;
        }

        // 5. move to the destination
        let copy_options = dir::CopyOptions::new();
        dir::move_dir(
            snapshot_path.clone(),
            &self.snapshots_target_directory,
            &copy_options,
        )?;

        // remove the tmp folder
        fs_extra::remove_items(&[snapshot_path])?;

        // 6. start the node container back up
        self.start().await?;

        Ok(())
    }

    pub async fn can_snapshot(&self) -> bool {
        match self.get_head().await {
            Ok(_) => {
                if let Some(instant) = self.last_snapshot_timestamp {
                    // TODO: make this optional and part of the configuration
                    // 86400 secs -> 24 hours
                    instant.elapsed() >= Duration::from_secs(30)
                } else {
                    true
                }
            },
            Err(_) => {
                // if the node does not respond to the rpc, do not snapshot
                // this catches a corner-case where, the node is started with a cleaned up DB
                // and is not yet ready for the first snapshot
                false
            },
        }
        // TODO: debug, remove
        // if let Some(instant) = self.last_snapshot_timestamp {
        //     // TODO: make this optional and part of the configuration
        //     // 86400 secs -> 24 hours
        //     instant.elapsed() >= Duration::from_secs(5)
        // } else {
        //     true
        // }
    }
}
