// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use slog::{error, info, warn, Drain, Level, Logger};
use tokio::signal;

pub mod configuration;
pub mod node;

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

    let mut node = TezedgeNode::new(
        tezedge_node_url,
        container_name,
        tezedge_database_directory,
        snapshots_target_directory,
    );

    let running = Arc::new(AtomicBool::new(true));
    let running_thread = running.clone();

    let handle = tokio::spawn(async move {
        while running_thread.load(std::sync::atomic::Ordering::Acquire) {
            if node.can_snapshot() {
                if let Err(e) = node.take_snapshot().await {
                    // TODO match errors
                    warn!(log, "{:?}", e);
                }
            } else {
                // TODO clean this up + config
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    });

    // wait for SIGINT
    signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c event");
    // info!(log, "Ctrl-c or SIGINT received!");

    // set running to false
    running.store(false, Ordering::Release);
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
