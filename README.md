# rdlock

[![Crates.io](https://img.shields.io/crates/v/rdlock.svg)](https://crates.io/crates/rdlock)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

A library for distributed redis locks written in rust.

Only supports redis through bb8 connection pools

## TODO

Types of locks we want implemented:
* Simple lock (wip) - just a simple distributed lock with no protected data
* Mutex
* RwLock

Work in progress.
