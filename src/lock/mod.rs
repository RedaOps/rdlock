use std::time::{Duration, Instant};

use redis::Client;
use tokio_stream::StreamExt;

use crate::{
    errors::RdLockError,
    utils::{get_redis_lock_location, local_ttl_check, redis_remove_lockfile, redis_set_nx},
    RdLockConnectionManager, RdLockResult,
};

/// The most basic type of lock. It holds no value and is atomic across all instances connected to
/// the same redis instance.
///
/// The lock will only get released automatically when the TTL period ends.
/// In order to unlock it early, just call the `.release()` function.
pub struct BasicLock {
    id: String,
    lock_file: String,

    ttl: u64,
    pool: RdLockConnectionManager,
    is_locked: bool,

    lock_time: Instant,
}

impl BasicLock {
    /// Creates a new instance of a simple lock in an unlocked state.
    ///
    /// `ttl` is in seconds, and it represents the lock's automatic expiry time
    pub fn new(pool: RdLockConnectionManager, id: &str, ttl: u64) -> Self {
        let (lock_file, _) = get_redis_lock_location(id);
        Self {
            id: id.to_string(),
            lock_file,
            ttl,
            pool,
            is_locked: false,
            lock_time: Instant::now(),
        }
    }

    /// Returns the lock id
    pub fn id(&self) -> String {
        self.id.clone()
    }

    /// Returns the lock TTL in seconds
    pub fn ttl(&self) -> u64 {
        self.ttl
    }

    /// Attempts to acquire the lock instantly, fails otherwise
    ///
    /// If the `BasicLock` is already locked by this instance, returns `Ok(())` instantly
    pub async fn try_lock(&mut self) -> RdLockResult {
        local_ttl_check!(self);
        if self.is_locked {
            return Ok(());
        }

        let res = redis_set_nx(
            self.pool.get_connection_manager(),
            self.ttl,
            &self.lock_file,
        )
        .await;

        if res.is_ok() {
            self.lock_time = Instant::now();
            self.is_locked = true;
        }
        res
    }

    /// Locks, yielding until the lock is available and acquired.
    /// This operation opens an additional redis connection.
    ///
    /// If the `BasicLock` is already locked by this instance, returns `Ok(())` instantly
    pub async fn lock(&mut self) -> RdLockResult {
        // Infinite timeout
        self.lock_timeout(Duration::from_secs(u64::MAX)).await
    }

    /// Locks, yielding until the lock is available and acquired, with a specified timeout.
    /// This operation opens an additional redis connection.
    ///
    /// If the `BasicLock` is already locked by this instance, returns `Ok(())` instantly
    pub async fn lock_timeout(&mut self, timeout: Duration) -> RdLockResult {
        local_ttl_check!(self);
        if self.is_locked {
            return Ok(());
        }

        let pubsub_client = Client::open(self.pool.get_connection_info())?;
        let mut pubsub_conn = pubsub_client.get_async_pubsub().await?;
        pubsub_conn
            .psubscribe(format!("*{}*", self.lock_file))
            .await?;

        if self.try_lock().await.is_ok() {
            return Ok(());
        }

        // Else, we need to wait for the lock to unlock
        let mut timeout_interval = tokio::time::interval(timeout);
        timeout_interval.tick().await;
        let mut message_stream = pubsub_conn.on_message();
        loop {
            tokio::select! {
                _ = timeout_interval.tick() => {
                    return Err(RdLockError::TimeoutReached);
                }

                msg = message_stream.next() => {
                    if let Some(msg) = msg {
                        let payload: String = msg.get_payload()?;
                        if (payload == "del" || payload == "expired") && self.try_lock().await.is_ok() {
                                return Ok(());
                        }
                    }
                }
            }
        }
    }

    /// Releases or unlocks the `BasicLock` early, before the TTL expires
    ///
    /// Returns `Err(RdLockError::LockNotAcquired)` if the lock was not acquired.
    pub async fn release(&mut self) -> RdLockResult {
        local_ttl_check!(self);
        if !self.is_locked {
            return Err(RdLockError::LockNotAcquired);
        }

        if redis_remove_lockfile(self.pool.get_connection_manager(), &self.lock_file).await {
            self.is_locked = false;
            Ok(())
        } else {
            Err(RdLockError::ConnectionError)
        }
    }

    /// Returns `true` if the `BasicLock` is locked inside this instance.
    ///
    /// Also performs a local TTL sanity check to see if that is the case
    pub fn is_locked(&mut self) -> bool {
        local_ttl_check!(self);
        self.is_locked
    }
}

#[cfg(test)]
mod tests {
    use crate::{test_utils, utils::redis_remove_lockfile};

    use super::*;

    #[tokio::test]
    async fn lock() {
        let conn = test_utils::get_redis_test_conn().await;
        let mut lock = BasicLock::new(conn.clone(), "test_lock", 3600);
        let mut lock2 = BasicLock::new(conn.clone(), "test_lock", 1);
        let mut lock3 = BasicLock::new(conn.clone(), "test_lock", 3600);

        // Clear lockfile from previous tests
        assert!(
            redis_remove_lockfile(conn.get_connection_manager(), "/rdlock/test_lock/.lock").await
        );

        // Acquire the lock
        assert!(!lock.is_locked());
        assert_eq!(lock.try_lock().await, Ok(()));
        assert!(lock.is_locked());

        // Try acquiring the lock from the 2nd instance
        assert_eq!(lock2.try_lock().await, Err(RdLockError::LockUnavailable));

        // Unlock first lock
        assert_eq!(lock.release().await, Ok(()));
        assert!(!lock.is_locked());

        // Try again
        assert_eq!(lock2.try_lock().await, Ok(()));
        assert!(lock2.is_locked());

        // Wait for ttl to pass
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(!lock2.is_locked());

        // Acquire lock 1 again
        assert_eq!(lock.try_lock().await, Ok(()));
        assert!(lock.is_locked());

        // Acquire lock2 right when releasing 1
        let l3_thread = tokio::spawn(async move {
            if lock2.lock().await.is_err() {
                return false;
            }

            if lock2.release().await.is_err() {
                return false;
            }

            true
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(lock.release().await.is_ok());
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(l3_thread.await.unwrap());
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Lock L2 again, and then instantly create race condition between l1 and l3
        // This also tests atomicity resilience
        let mut lock2 = BasicLock::new(conn.clone(), "test_lock", 2);
        assert_eq!(lock2.try_lock().await, Ok(()));
        let l1_thread =
            tokio::spawn(async move { lock.lock_timeout(Duration::from_secs(5)).await });
        let l3_thread =
            tokio::spawn(async move { lock3.lock_timeout(Duration::from_secs(5)).await });
        tokio::time::sleep(Duration::from_secs(3)).await;

        assert!(
            (l1_thread.is_finished() || l3_thread.is_finished())
                && !(l1_thread.is_finished() && l3_thread.is_finished())
        );

        let hanging_thread = if l1_thread.is_finished() {
            l3_thread
        } else {
            l1_thread
        };

        assert_eq!(
            hanging_thread.await.unwrap(),
            Err(RdLockError::TimeoutReached)
        );

        // Try cleanup
        let _ =
            redis_remove_lockfile(conn.get_connection_manager(), "/rdlock/test_lock/.lock").await;
    }

    #[tokio::test]
    async fn crate_example_test() {
        let manager = RdLockConnectionManager::new("redis://localhost")
            .await
            .unwrap();
        let manager2 = manager.clone();
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);
        redis_remove_lockfile(manager.get_connection_manager(), "/rdlock/my_lock/.lock").await;

        // This is clone safe
        let manager_clone = manager.clone();
        let tx_clone = tx.clone();
        let h1 = tokio::spawn(async move {
            let mut basic_lock = manager_clone.new_basic_lock("my_lock", 3600);

            basic_lock.lock().await.unwrap();
            let _ = tx_clone.send("a").await;

            tokio::time::sleep(Duration::from_secs(5)).await;

            let _ = tx_clone.send("r").await;
            basic_lock.release().await.unwrap();
        });

        let h2 = tokio::spawn(async move {
            let mut basic_lock = manager.new_basic_lock("my_lock", 3600);

            basic_lock.lock().await.unwrap();
            let _ = tx.send("a").await;

            tokio::time::sleep(Duration::from_secs(5)).await;

            let _ = tx.send("r").await;
            basic_lock.release().await.unwrap();
        });

        let _ = h1.await;
        let _ = h2.await;

        redis_remove_lockfile(manager2.get_connection_manager(), "/rdlock/my_lock/.lock").await;

        let mut buffer = Vec::new();
        rx.recv_many(&mut buffer, 4).await;

        assert_eq!(buffer, vec!["a", "r", "a", "r"]);
    }
}
