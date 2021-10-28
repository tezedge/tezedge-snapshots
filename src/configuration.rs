// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use clap::{App, Arg};
use std::env;

use url::Url;

#[derive(Clone, Debug)]
pub struct TezedgeSnapshotEnvironment {
    // logging level
    pub log_level: slog::Level,

    // interval in seconds to check for the head of the node
    pub head_check_interval: u64,

    // the url to the node's rpc server
    pub tezedge_node_url: Url,

    // name of the container the tezedge node resides in
    pub container_name: String,

    // TODO: add options for snapshot frequency in blocks
    // TODO: add options for snapshot frequency: daily, weekly, ... Note: in combination of timestamp?
    // TODO: add options for concrete levels to snapshot on
}

fn tezedge_snapshots_app() -> App<'static, 'static> {
    let app = App::new("Tezedge snapshotting app")
        .version(env!("CARGO_PKG_VERSION"))
        .author("TezEdge and the project contributors")
        .setting(clap::AppSettings::AllArgsOverrideSelf)
        .arg(
            Arg::with_name("container-name")
                .long("container-name")
                .takes_value(true)
                .value_name("STRING")
                .help("The name of the container the tezedge node resides in"),
        )
        .arg(
            Arg::with_name("tezedge-node-url")
                .long("tezedge-node-url")
                .takes_value(true)
                .value_name("URL")
                .help("The url to the tezedge node for the snapshots"),
        )
        .arg(
            Arg::with_name("head-check-interval")
                .long("head-check-interval")
                .takes_value(true)
                .value_name("U64")
                .help("Interval in seconds to take check the node's head"),
        )
        .arg(
            Arg::with_name("log-level")
                .long("log-level")
                .takes_value(true)
                .value_name("SLOG LEVEL")
                .possible_values(&["critical", "error", "warn", "info", "debug", "trace"])
                .help("Set logging level"),
        );

    app
}

impl TezedgeSnapshotEnvironment {
    pub fn from_args() -> Self {
        let app = tezedge_snapshots_app();
        let args = app.clone().get_matches();

        Self {
            log_level: args
                .value_of("log-level")
                .unwrap_or("info")
                .parse::<slog::Level>()
                .expect("Was expecting one value from slog::Level"),

            head_check_interval: args
                .value_of("head-check-interval")
                .unwrap_or("5")
                .parse::<u64>()
                .expect("Expected u64 value of seconds"),

            tezedge_node_url: args
                .value_of("tezedge-node-url")
                .unwrap_or("http://localhost:18732")
                .parse::<Url>()
                .expect("Was expecting a valid url"),
            container_name: args
                .value_of("container-name")
                .unwrap_or("tezedge-node")
                .to_string()
        }
    }
}