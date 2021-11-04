// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use slog::{error, info, warn, Drain, Level, Logger};
use tokio::{signal, time};

pub mod configuration;
pub mod node;

use crate::configuration::TezedgeSnapshotEnvironment;
use crate::node::{TezedgeNodeController, TezedgeNodeControllerError};

#[tokio::main]
async fn main() {
    let env = TezedgeSnapshotEnvironment::from_args();

    let TezedgeSnapshotEnvironment {
        log_level,
        tezedge_node_url,
        check_interval,
        node_container_name,
        monitoring_container_name,
        tezedge_database_directory,
        snapshots_target_directory,
        snapshot_capacity,
        snapshot_frequency,
    } = env;

    // create an slog logger
    let log = create_logger(log_level);

    let mut node = TezedgeNodeController::new(
        tezedge_node_url,
        node_container_name,
        monitoring_container_name,
        tezedge_database_directory,
        snapshots_target_directory,
        log.clone(),
    );

    let running = Arc::new(AtomicBool::new(true));

    let running_thread = running.clone();
    let thread_log = log.clone();
    let handle = tokio::spawn(async move {
        while running_thread.load(std::sync::atomic::Ordering::Acquire) {
            if node.can_snapshot(snapshot_frequency).await {
                info!(thread_log, "Taking new snapshot");
                if let Err(e) = node.take_snapshot(snapshot_capacity).await {
                    match e {
                        TezedgeNodeControllerError::NodeUnreachable => warn!(thread_log, "{:?}", e),
                        _ => {
                            error!(thread_log, "{:?}", e);
                            break;
                        }
                    }
                }
            } else {
                time::sleep(time::Duration::from_secs(check_interval)).await;
            }
        }
    });

    // wait for SIGINT
    signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c event");
    info!(log, "Ctrl-c or SIGINT received!");

    // set running to false
    running.store(false, Ordering::Release);

    drop(handle);
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
