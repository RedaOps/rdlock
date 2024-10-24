# rdlock

A library for distributed redis locks written in rust.

Only supports redis through bb8 connection pools

## TODO

Types of locks we want implemented:
* Simple lock (wip) - just a simple distributed lock with no protected data
* Mutex
* RwLock

Work in progress.
