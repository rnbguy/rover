# Rover for Cosmos Network

Send **rover** to different Cosmos _chains_ and perform queries and broadcast transactions.

## Installation

```
cargo install --git https://github.com/rnbguy/rover
```

## Setup Hasura GraphQL

```sh
rover config graphql GRAPHQL_ENDPOINT
```

More info at [map of zones docs](https://docs.mapofzones.com/graphql.html).

## Usage

```sh
rover add-key-to-os my_priv_key
secret-tool lookup application rust-keyring service rover username my_priv_key
```

```sh
PRIV_KEY=$(secret-tool lookup application rust-keyring service rover username my_priv_key) cargo install --release
```

```sh
rover add-account Os:my_priv_key my_account
# or
rover add-account Memory:mem_key my_account
# or
rover add-account Ledger my_account
```

```sh
rover tx sentinelhub-2 [grantee_address] restake my_account
rover tx cosmoshub-4 [grantee_address] vote my_account 1:Yes 2:Abstain 3:No
```

## Features

- [x] Wallet support
  - [x] OS Keystore via [keyring](https://lib.rs/crates/keyring)
  - [x] Memory backend
  - [x] Ledger via [ledger-transport](https://lib.rs/crates/ledger-transport)
- [x] All types of endpoint support
  - [x] RPC
  - [x] GRPC
  - [x] REST
- [x] Gas simulation
- [x] List public endpoints
- [x] Pull chain info from online resources
  - [x] PingPub
  - [x] Cosmos directory (`cosmos/chain-registry` clone)
  - [x] Emeris (closed)
- [x] CLI
  - [ ] Transaction
    - [x] Send
    - [x] Vote
    - [x] Delegate
    - [x] Redelegate
    - [x] Auto-restake
    - [x] Grant
    - [x] IBC transfer
    - [x] Support Authz exec
    - [ ] ??
  - [ ] Query
    - [ ] ??
- [ ] TUI
  - [ ] ??
