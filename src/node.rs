// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use url::{Url, ParseError};
use serde::Deserialize;
use thiserror::Error;
use shiplift::Docker;
use std::{path::{Path, PathBuf}, time::Duration, vec};
use fs_extra::dir;
use chrono::Utc;
use chrono::Timelike;

#[derive(Clone, Debug, Deserialize)]
pub struct TezosBlockHeader {
    protocol: String,
    chain_id: String,
    hash: String,
    level: i32,
    proto: i32, // TODO: verify
    predecessor: String,
    timestamp: String, // TODO: rc-3999 format? Verify
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
}

impl TezedgeNode {
    pub fn new(url: Url, container_name: String, database_directory: PathBuf) -> Self {
        Self {
            url,
            container_name,
            database_directory,
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

        docker.containers().get(&self.container_name).stop(Some(Duration::from_secs(10))).await?;

        Ok(())
    }

    /// Starts the tezedge container
    pub async fn start(&self) -> Result<(), TezedgeNodeError> {
        let docker = Docker::new();

        docker.containers().get(&self.container_name).start().await?;

        // TODO: preform a health check with get_head

        Ok(())
    }

    /// Takes a snapshot of the tezedge node
    pub async fn take_snapshot(&self, /*snapshot_block_header: TezosBlockHeader*/) -> Result<(), TezedgeNodeError> {

        // 1. stop the node container
        // self.stop().await?;

        // get the time for the snapshot title
        let now = Utc::now().naive_utc();
        println!("Naive utc: {}", now.to_string());
        let date = now.date().to_string().replace('-', "");
        let time: String = now.time().to_string().replace(':', "").split('.').take(1).collect();

        println!("date: {} time: {}", date, time);
        // 2. copy out the database directories to a temp folder
        // let copy_options = dir::CopyOptions::new();
        // let temp_destination = Path::new("/tmp/tezedge-snapshots-tmp");

        // dir::copy(&self.database_directory, temp_destination, &copy_options)?;

        // // 3. remove identity, log files and lock files
        // let to_remove = vec![temp_destination.join("identity.json"), temp_destination.join("*.log"), temp_destination.join("context/index/lock")];
        // fs_extra::remove_items(&to_remove)?;

        // // 4. zip/tar it?
        // // TODO

        // // 5. move to the destination
        // dir::move_dir(temp_destination, &self.snapshots_target_directory, &copy_options)?;

        // // 6. start the node container back up
        // self.start().await?;

        Ok(())
    }
}