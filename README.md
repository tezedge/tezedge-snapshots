# Tezedge snapshots

Easy docker application for taking tezedge snapshots in a set interval. The application is designed to be run as a service inside a docker-compose. The compose should at least include a tezedge node and the snapshotting app. See the included [docker-compose](docker-compose.yml).

## Prerequisites

Installed docker and docker-compose.

## Snapshots

tezedge_\<network_name\>_\<date\>-\<time\>_\<block_hash\>_\<context_type\>.\<snapshot_type\>

### Example

`tezedge_granadanet_20211108-104156_BLo9BSrp7S8HnrX43vK3LdHpHUAoTVSqFACtzczjfP7a2CExUZe_irmin.archive`

The snapshot above comes from `granadanet` and was taken on the `8th of November 2021` at `10:41:56 UTC` and at block `BLo9BSrp7S8HnrX43vK3LdHpHUAoTVSqFACtzczjfP7a2CExUZe`. The snapshot was taken on a node running with the `irmin` context and the snapshot type is `archive`.

## Running

1. Clone this repository

```
git clone https://github.com/tezedge/tezedge-snapshots.git
```

2. Set a few environmental variables. (This step is optional as you can set the environment variables before executing the command)

```
export NODE_HOSTNAME_OR_IP=<public IP address of the machine you run the snapshotter or its domain>
export TEZOS_NETWORK=<tezos network to connect to>
export TEZEDGE_VOLUME_PATH=<path to the tezedge databases>
export TEZEDGE_SNAPSHOTS_VOLUME_PATH=<path to the directory you want your snapshots saved to>
```

3. Run the docoker-compose

```
docker-compose -f docker-compose.yml up -d
```

4. Alternatively, you can combine step 2 and 3 into a single one liner

```
NODE_HOSTNAME_OR_IP=116.202.128.230 TEZOS_NETWORK=granadanet TEZEDGE_VOLUME_PATH="/path/to/tezedge" TEZEDGE_SNAPSHOTS_VOLUME_PATH="/path/to/snapshots"  docker-compose -f docker-compose.yml up -d
```

## Options

- `snapshots-target-directory`: The path to the target directory for the snapshots
- `tezedge-database-directory`: The path to the running tezedge node database directory
- `check-interval`: Interval in seconds to take check the node's head
- `snapshot-frequency`: The time between two snapshots in seconds
- `snapshot-capacity`: The maximum number of snapshots kept on the machine
- `tezedge-node-url`: The url to the tezedge node for the snapshots
- `network`: The name of network tezedge is connecting to
- `node-container-name`: The name of the container the tezedge node resides in
- `monitoring-container-name`: The name of the container the tezedge monitoring resides in
- `log-level`: Set logging level
- `snapshot-type`: The type of the snapshot to take. One of the following values: archive, full, all (take all snapshot types)
- `full-snapshot-image`: The tezedge image used to create the full snapshot. Defaults to: "tezedge/tezedge:latest"

## Nginx file server configuration

```
server {
        listen 80 default_server;
        listen [::]:80 default_server;
        add_header Access-Control-Allow-Origin *;

        root /var/www/html;

        # Add index.php to the list if you are using PHP
        index index.html index.htm index.nginx-debian.html;

        server_name _;

        location / {
                root /snapshots/;
                autoindex on;
                autoindex_format json;
        }
}
```