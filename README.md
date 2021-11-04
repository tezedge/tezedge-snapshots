# Tezedge snapshots

Easy docker application for taking tezedge snapshots in a set interval. The application is designed to be run as a service inside a docker-compose. The compose should at least include a tezedge node and the snapshotting app. See the included [docker-compose](docker-compose.yml).

## Prerequisites

Installed docker and docker-compose.

## Snapshots

tezedge-\<network_name\>-\<date\>-\<time\>-\<block_level\>

### Example

`tezedge-granadanet-20211104-135756-230849`

The snapshot above comes from `granadanet` and was taken on the `4th of November 2021` at `13:57:56 UTC` and at block level `230849`.

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