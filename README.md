# Scuttle-chat

**[Work In Progress!!]** Ephemeral chats over encrypted TCP streams for the Scuttleverse

Built with <3 by @corlock `@sHFNLAao6phQ5AN17ecYNUbszDa4Qf6DhyQsjtQfdmY=.ed25519`

## What it do

Scuttle-chat is a p2p chat application in a terminal UI. It makes use of Scuttlebutt's identity system for its public key infrastructure, and attempts to resolve SSB aliases/usernames of peers via your local SSB database.

**Current Features**
- [x] LAN based peer discovery (via UDP broadcast, akin to SSB's peer discovery but on a different port)
- [x] Identities are verified with [Secret Handshake](https://ssbc.github.io/scuttlebutt-protocol-guide/#handshake)
- [x] Chats are directly peer-to-peer, encrypted using SSB's [BoxStream](https://ssbc.github.io/scuttlebutt-protocol-guide/#box-stream) bulk encryption protocol
- [x] Chat history persisted over entire application lifecycle (independent of connection dropouts)

**To do**
- [ ] Source keypair from ~/.ssb/secret
- [ ] Tests!
- [ ] Resolving of username/aliases from local SSB database when available
- [ ] Ability to manually set unverified username on startup for non-scuttlebutt users
- [ ] Add cursor support
- [ ] Add CLI arguments for customization (debug info, custom port selection, etc.)
- [ ] Improve debug log / window
- [ ] Improve scroll behavior in chat window
- [ ] Integrate with [ssb rooms](https://github.com/staltz/ssb-room)

## Install & Run

Build & run via Cargo. Make sure you have Rust and Cargo [installed](https://www.rust-lang.org/tools/install).

```
cargo build
cargo run
```

## Motivation

[Scuttlebutt](https://scuttlebutt.nz) is really good at a bunch of things. Its biggest win is arguably its social graph, which creates a decentralized trusted network of public keys. In no other ecosystem do you have a fully decentralized Public Key Infrastructure where the trust signals that "Alice" is "Alice" come purely from her own history of messages, media and posts, combined with the trust signas from other trusted friends following Alice.

Today, if users of Scuttlebutt want to chat through means other than Scuttlebutt itself, they lose all verification of the identity system that SSB relies on. Wouldn't it be nice to have realtime communication tools (chat, p2p video/audio calls, etc.) that make use of Scuttlebutt for its identity and peer discovery, but do their own communication out-of-band of Scuttlebutt's main "feed based" architecture for propogating messages?

This project serves as an MVP for experimenting with these ideas.
