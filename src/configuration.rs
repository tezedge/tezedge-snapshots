// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use clap::{App, Arg};
use std::{
    env,
    path::{Path, PathBuf},
    str::FromStr,
};

use url::Url;

#[derive(Clone, Debug)]
pub struct TezedgeSnapshotEnvironment {
    // logging level
    pub log_level: slog::Level,

    // interval in seconds to perform the check for can_snapshot
    pub check_interval: u64,

    // the url to the node's rpc server
    pub tezedge_node_url: Url,

    // name of the container the tezedge node resides in
    pub node_container_name: String,

    // name of the container the tezedge monitoring resides in
    pub monitoring_container_name: String,

    // path to the target directory for the snapshots
    pub snapshots_target_directory: PathBuf,

    // path to the running tezedge node database directory
    pub tezedge_database_directory: PathBuf,

    // maximum number of snapshots kept on the machine
    pub snapshot_capacity: usize,

    // frequency of the snapshots in seconds
    pub snapshot_frequency: u64,

    // network tezedge is connecting to
    pub network: String,

    // what snapshots to create
    pub snapshot_type: SnapshotType,

    /// use this image to create the full snapshotting container
    pub full_snapshot_image: String,

    pub context_type: ContextType,

    // TODO: add options for snapshot frequency in blocks
    // TODO: add options for snapshot frequency: daily, weekly, ... Note: in combination of timestamp?
    // TODO: add options for concrete levels to snapshot on
}

#[derive(Clone, Debug)]
pub enum SnapshotType {
    Archive,
    Full,
    All,
}

#[derive(Clone, Debug)]
pub enum ContextType {
    Irmin,
    Tezedge,
}

#[derive(Clone, Debug)]
pub struct TypeNotFound {}

impl FromStr for SnapshotType {
    type Err = TypeNotFound;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "archive" => Ok(SnapshotType::Archive),
            "full" => Ok(SnapshotType::Full),
            "all" => Ok(SnapshotType::All),
            _ => Err(TypeNotFound {}),
        }
    }
}

impl FromStr for ContextType {
    type Err = TypeNotFound;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "irmin" => Ok(ContextType::Irmin),
            "tezedge" => Ok(ContextType::Tezedge),
            _ => Err(TypeNotFound {}),
        }
    }
}

impl ToString for ContextType {
    fn to_string(&self) -> String {
        match self {
            ContextType::Irmin => String::from("irmin"),
            ContextType::Tezedge => String::from("tezedge"),
        }
    }
}

fn tezedge_snapshots_app() -> App<'static, 'static> {
    let app = App::new("Tezedge snapshotting app")
        .version(env!("CARGO_PKG_VERSION"))
        .author("TezEdge and the project contributors")
        .setting(clap::AppSettings::AllArgsOverrideSelf)
        .arg(
            Arg::with_name("tezedge-database-directory")
                .long("tezedge-database-directory")
                .takes_value(true)
                .value_name("PATH")
                .help("The path to the running tezedge node database directory")
                .validator(|p| {
                    if Path::new(&p).exists() {
                        Ok(())
                    } else {
                        Err(format!("Database directory path not found '{}'", p))
                    }
                }),
        )
        .arg(
            Arg::with_name("snapshots-target-directory")
                .long("snapshots-target-directory")
                .takes_value(true)
                .value_name("PATH")
                .help("The path to the target directory for the snapshots")
                .validator(|p| {
                    if Path::new(&p).exists() {
                        Ok(())
                    } else {
                        Err(format!("Snapshot target directory path not found '{}'", p))
                    }
                }),
        )
        .arg(
            Arg::with_name("node-container-name")
                .long("node-container-name")
                .takes_value(true)
                .value_name("STRING")
                .help("The name of the container the tezedge node resides in"),
        )
        .arg(
            Arg::with_name("network")
                .long("network")
                .takes_value(true)
                .value_name("STRING")
                .help("The name of network tezedge is connecting to"),
        )
        .arg(
            Arg::with_name("monitoring-container-name")
                .long("monitoring-container-name")
                .takes_value(true)
                .value_name("STRING")
                .help("The name of the container the tezedge monitoring resides in"),
        )
        .arg(
            Arg::with_name("tezedge-node-url")
                .long("tezedge-node-url")
                .takes_value(true)
                .value_name("URL")
                .help("The url to the tezedge node for the snapshots"),
        )
        .arg(
            Arg::with_name("snapshot-capacity")
                .long("snapshot-capacity")
                .takes_value(true)
                .value_name("USIZE")
                .help("The maximum number of snapshots kept on the machine"),
        )
        .arg(
            Arg::with_name("snapshot-frequency")
                .long("snapshot-frequency")
                .takes_value(true)
                .value_name("U64")
                .help("The frequency of the snapshots in seconds"),
        )
        .arg(
            Arg::with_name("check-interval")
                .long("check-interval")
                .takes_value(true)
                .value_name("U64")
                .help("The interval in seconds to perform the check for can_snapshot"),
        )
        .arg(
            Arg::with_name("snapshot-type")
                .long("snapshot-type")
                .takes_value(true)
                .value_name("SnapshotType")
                .help("Type of the snapshots"),
        )
        .arg(
            Arg::with_name("context-type")
                .long("context-type")
                .takes_value(true)
                .value_name("ContextType")
                .help("Type of the context"),
        )
        .arg(
            Arg::with_name("full-snapshot-image")
                .long("full-snapshot-image")
                .takes_value(true)
                .value_name("STRING")
                .help("The name of the tezedge image to use for the full snapshots"),
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

            check_interval: args
                .value_of("check-interval")
                .unwrap_or("5")
                .parse::<u64>()
                .expect("Expected u64 value of seconds"),

            tezedge_node_url: args
                .value_of("tezedge-node-url")
                .unwrap_or("http://localhost:18732")
                .parse::<Url>()
                .expect("Was expecting a valid url"),
            node_container_name: args
                .value_of("node-container-name")
                .unwrap_or("tezedge-node")
                .to_string(),
            monitoring_container_name: args
                .value_of("monitoring-container-name")
                .unwrap_or("tezedge-node-monitoring")
                .to_string(),
            network: args.value_of("network").unwrap_or("network").to_string(),
            snapshots_target_directory: args
                .value_of("snapshots-target-directory")
                .unwrap_or("/tmp/snapshots")
                .parse::<PathBuf>()
                .expect("The provided path is invalid"),
            tezedge_database_directory: args
                .value_of("tezedge-database-directory")
                .unwrap_or("/tmp/tezedge")
                .parse::<PathBuf>()
                .expect("The provided path is invalid"),
            snapshot_capacity: args
                .value_of("snapshot-capacity")
                .unwrap_or("7")
                .parse::<usize>()
                .expect("Expected usize value"),
            snapshot_frequency: args
                .value_of("snapshot-frequency")
                .unwrap_or("86400")
                .parse::<u64>()
                .expect("Expected u64 value"),
            snapshot_type: args
                .value_of("snapshot-type")
                .unwrap_or("all")
                .parse::<SnapshotType>()
                .expect("Expected values archive, full or all"),
            full_snapshot_image: args
                .value_of("full-snapshot-image")
                .unwrap_or("tezedge/tezedge:latest")
                .to_string(),
            context_type: args
                .value_of("context-type")
                .unwrap_or("irmin")
                .parse::<ContextType>()
                .expect("Expected values archive, full or all"),
        }
    }
}
