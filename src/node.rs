// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt::format;

use url::{Url, ParseError};
use reqwest::blocking::Client;
use serde::Deserialize;
use thiserror::Error;
use shiplift::Docker;
use std::time::Duration;

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
}

#[derive(Debug, Error)]
pub enum TezedgeNodeError {
    #[error("The defined tezedge node is unreachable")]
    NodeUnreachable,
    #[error("Failed to parse url: {0}")]
    MalformedUrl(#[from] ParseError),
    #[error("Request to the node failed: {0}")]
    FailedRequest(#[from] reqwest::Error)
}

impl TezedgeNode {
    pub fn new(url: Url, container_name: String) -> Self {
        Self {
            url,
            container_name
        }
    }

    // TODO: start, stop
    pub fn get_head(&self) -> Result<TezosBlockHeader, TezedgeNodeError> {
        let header_url = self.url.join("chains/main/blocks/head/header")?;
        let head_header = reqwest::blocking::get(header_url)?.json()?;

        // print!("HEADER: {:?}", head_header);

        Ok(head_header)
    }

    pub async fn stop(&self) -> Result<(), TezedgeNodeError> {
        let docker = Docker::new();

        let res = docker.containers().get(&self.container_name).stop(Some(Duration::from_secs(10))).await;

        Ok(())
    }

    pub async fn start(&self) -> Result<(), TezedgeNodeError> {
        let docker = Docker::new();

        let res = docker.containers().get(&self.container_name).start().await;

        Ok(())
    }
}