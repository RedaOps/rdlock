# rdlock

[![Crates.io](https://img.shields.io/crates/v/rdlock.svg)](https://crates.io/crates/rdlock)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

A library for distributed redis locks written in rust.

These are some useful synchronization primitives which work among multiple separate processes, servers and clusters, as long as they communicate with the same redis server.

The redis instance must have `notify-keyspace-events` enabled with at least `KA` flags.
If this is too much overhead for your current instance, consider running a separate redis instance just for the distributed locks.

Usage and examples on [docs.rs](https://docs.rs/rdlock/latest/rdlock/)

## TLDR
It uses redis SET with the NX option enabled for creating a "lockfile".

Any `lock` function that waits for the lock to become available (e.g: `.lock()` or `.lock_timeout()`) will use redis pubsub in order to monitor for changes and try to acquire the "lockfile" once it is available.

## Known issues
* The local TTL check, which checks if the TTL of the specific lock expired before doing any extra actions, is not perfect, since it's only a local sanity TTL check, and doesn't verify TTL in any way with the redis instance.
* If the `pubsub` drops while waiting for a lock and/or we miss a message, we will never acquire it, since redis pubsub is fire and forget.

## Supported locks
* `BasicLock` - a basic type of distributed lock with no protected data. This is the most basic synchronization primitive.

## TODO
* Move generic lock functions that will be used in all locks out of `src/lock/mod.rs`

Types of locks we want implemented:
* Mutex
* RwLock

## Footnotes
Please note this is a work in progress and for starters I will be implementing only what I need or want to have fun with. If there is a certain synchronization primitive that is not available and you need it or you want to implement it for fun, contributions are always welcome :)
