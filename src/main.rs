// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::{error, info, Drain, Level, Logger};

pub mod node;
pub mod configuration;

use crate::configuration::TezedgeSnapshotEnvironment;
use crate::node::TezedgeNode;

#[tokio::main]
async fn main() {
    let env = TezedgeSnapshotEnvironment::from_args();

    let TezedgeSnapshotEnvironment {
        log_level,
        tezedge_node_url,
        head_check_interval,
        container_name,
        tezedge_database_directory,
        snapshots_target_directory,
        snapshot_capacity,
    } = env;

    // create an slog logger
    let log = create_logger(log_level);

    let node = TezedgeNode::new(tezedge_node_url, container_name, tezedge_database_directory);

    //TODO: cycle
    // match node.get_head() {
    //     Ok(header) => slog::info!(log, "{:?}", header),
    //     Err(e) => slog::warn!(log, "Some error: {:?}", e)
    // }

    // Test wether we can stop a container from another contianer
    // node.stop().await.expect("Failed to stop the container");
    // tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    // node.start().await.expect("Failed to start the contianer");
    // tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    // node.stop().await.expect("Failed to stop the container");

    match node.take_snapshot().await {
        Ok(()) => info!(log, "OK"),
        Err(e) => error!(log, "Error: {:?}", e)
    }

}

/// Creates a slog Logger
fn create_logger(level: Level) -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
            .build()
            .fuse(),
    )
    .chan_size(32768)
    .overflow_strategy(slog_async::OverflowStrategy::Block)
    .build()
    .filter_level(level)
    .fuse();
    Logger::root(drain, slog::o!())
}