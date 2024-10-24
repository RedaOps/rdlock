use anyhow::Result;

use crate::{LockState, RedisConnectionPool};

/// The most basic type of lock. It holds no value and is atomic across all instances connected to
/// the same redis instance.
///
/// The lock will only get released automatically when the TTL period ends.
/// In order to unlock it early, just call the `.unlock()` function.
pub struct Lock {
    id: String,
    ttl: u64,
    bb8_pool: RedisConnectionPool,
    state: LockState,
}

impl Lock {
    /// Creates a new instance of a simple lock in an unlocked state.
    pub fn new(bb8_pool: RedisConnectionPool, id: String, ttl: u64) -> Self {
        Self {
            id,
            ttl,
            bb8_pool,
            state: LockState::default(),
        }
    }

    /// Attempts to aquire the lock instantly, fails otherwise
    ///
    /// If the lock is already locked by this instance, returns Ok(()) instantly
    pub async fn try_lock(&self) -> Result<()> {
        let mut conn = self.bb8_pool.get().await?;
        todo!("implement")
    }

    /// Locks, yielding until the lock is available and acquired.
    ///
    /// If the lock is already locked by this instance, returns Ok(()) instantly
    pub async fn lock(&self) -> Result<()> {
        todo!("implement")
    }

    /// Unlocks the simple lock early, before the TTL expires
    pub async fn unlock(&self) -> Result<()> {
        todo!("implement")
    }
}
