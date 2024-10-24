//!  rdlock: A library for distributed redis locks written in rust.
//!
//!  Before using, make sure your redis instance has `notify-keyspace-events` enabled with `KA`
//!  flags.
//!
//!  # Example with a basic lock
//!
//!  Please note, this should never be used for locks between multiple threads/tasks in the same process,
//!  we have [blocking](https://doc.rust-lang.org/std/sync/index.html) and [non-blocking](https://docs.rs/tokio/latest/tokio/sync/index.html) primitives for those situations that are already available and well-optimized, so please use those when possible.
//!
//!  This library is useful when we need to synchronize across multiple processes with a local redis server,
//!  or, more importantly, across multiple machines, e.g. pods inside a cluster.
//!
//!  In this example, we could consider `h1` machine A and `h2` machine B, connecting to a "remote"
//!  redis instance.
//!
//!  ```rust,no_run
//!  use rdlock::RdLockConnectionManager;
//!  use std::time::Duration;
//!
//!  async fn do_work(manager: RdLockConnectionManager) {
//!     println!("Started thread");
//!     let mut basic_lock = manager.new_basic_lock("my_lock", 3600);
//!
//!     basic_lock.lock().await.unwrap();
//!     println!("Acquired lock");
//!
//!     // Do stuff
//!     tokio::time::sleep(Duration::from_secs(5)).await;
//!
//!     println!("Releasing lock");
//!     basic_lock.release().await.unwrap();
//!
//!  }
//!
//!  #[tokio::main]
//!  pub async fn main() {
//!     let manager = RdLockConnectionManager::new("redis://localhost").await.unwrap();
//!
//!     // Both machines will use the same `lock_id`, so they will target the same lock.
//!
//!     // This is clone safe
//!     let manager_clone = manager.clone();
//!     let h1 = tokio::spawn(async move {
//!         do_work(manager_clone).await
//!     });
//!
//!     let manager_clone = manager.clone();
//!     let h2 = tokio::spawn(async move {
//!         do_work(manager_clone).await
//!     });
//!
//!     let _ = h1.await;
//!     let _ = h2.await;
//!  }
//!  ```
//!
//!  You should always get something like this:
//!  ```text
//!  Started thread
//!  Started thread
//!  Acquired lock
//!  Releasing lock
//!  Acquired lock
//!  Releasing lock
//!  ```
//!
//!  And **never** something like this:
//!  ```text
//!  Started thread
//!  Started thread
//!  Acquired lock
//!  Acquired lock
//!  Releasing lock
//!  Releasing lock
//!  ```
//!  # Lock ID
//!
//!  All **unique** locks should use unique ids. This means that you should use the same `lock_id`
//!  across instances when targeting the same lock, but a different `lock_id` when you want to target a different
//!  lock.
//!
//!  ```rust,no_run
//!  # use rdlock::RdLockConnectionManager;
//!  # let rt = tokio::runtime::Runtime::new().unwrap();
//!  # rt.block_on(async {
//!  // Machine A
//!  let machine_a_manager = RdLockConnectionManager::new("<redis remote url>").await.unwrap();
//!
//!  // Targets the same lock as `machine_b_dog_lock`
//!  let machine_a_dog_lock = machine_a_manager.new_basic_lock("dog", 3600);
//!  // Targets the same lock as `machine_b_cat_lock`
//!  let machine_a_cat_lock = machine_a_manager.new_basic_lock("cat", 3600);
//!
//!
//!  // Machine B
//!  let machine_b_manager = RdLockConnectionManager::new("<redis remote url>").await.unwrap();
//!
//!  // Targets the same lock as `machine_a_dog_lock`
//!  let machine_b_dog_lock = machine_b_manager.new_basic_lock("dog", 3600);
//!  // Targets the same lock as `machine_a_cat_lock`
//!  let machine_b_cat_lock = machine_b_manager.new_basic_lock("cat", 3600);
//!  # })
//!  ````

use errors::RdLockError;
use lock::BasicLock;
use redis::{aio::ConnectionManager, Client, IntoConnectionInfo};
use utils::{redis_get_config, LockVersion};

pub mod errors;
pub mod lock;
pub mod utils;

pub(crate) type RdLockResult = Result<(), RdLockError>;

pub(crate) const DEFAULT_LOCK_VERSION: LockVersion = LockVersion::V1;

/// The `RdLockConnectionManager` is used to connect to a redis instance and initialize locks.
///
/// This is a wrapper on redis's original [`ConnectionManager`](https://docs.rs/redis/latest/redis/aio/struct.ConnectionManager.html)
/// that allows locks in this library to use both a multiplexed connection for basic operations,
/// but also to open separate connections
/// for pubsubs when awaiting lock availability.
/// It is clone safe (cloning does not open additional connections)
/// and can be passed to multiple threads/objects safely.
///
/// This structure is used to initialize the redis distributed locks implemented in this crate.
///
/// # Examples
///
/// ```rust,no_run
/// use rdlock::RdLockConnectionManager;
///
/// #[tokio::main]
/// pub async fn main() {
///     let rd_manager = RdLockConnectionManager::new("redis://localhost").await.unwrap();
///     let mut basic_lock = rd_manager.new_basic_lock("test_lock", 3600);
/// }
/// ````
#[derive(Clone)]
pub struct RdLockConnectionManager {
    conn: ConnectionManager,
    creds: redis::ConnectionInfo,
}

impl RdLockConnectionManager {
    /// Creates a new instance and connects to the redis server.
    ///
    /// This also checks  if the redis instance supports distributed locks.
    pub async fn new<T: IntoConnectionInfo>(params: T) -> Result<Self, RdLockError> {
        let creds = params.into_connection_info()?;
        let mut conn = ConnectionManager::new(Client::open(creds.clone())?).await?;
        Self::init_check_redis(&mut conn).await?;
        Ok(Self { creds, conn })
    }

    async fn init_check_redis(conn: &mut ConnectionManager) -> RdLockResult {
        let nke = redis_get_config(conn, "notify-keyspace-events").await?;
        if nke.contains("K") && nke.contains("A") {
            Ok(())
        } else {
            Err(RdLockError::InitCheckFailed)
        }
    }

    /// Creates a new [`BasicLock`]
    ///
    /// `ttl` is in seconds, and it represents the lock's automatic expiry time
    pub fn new_basic_lock(&self, lock_id: &str, ttl: u64) -> BasicLock {
        BasicLock::new(self.clone(), lock_id, ttl)
    }

    /// Returns a clone of the `ConnectionManager` inside.
    pub fn get_connection_manager(&self) -> ConnectionManager {
        self.conn.clone()
    }

    /// Returns a clone of the redis connection info inside.
    pub fn get_connection_info(&self) -> redis::ConnectionInfo {
        self.creds.clone()
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use crate::RdLockConnectionManager;

    pub async fn get_redis_test_conn() -> RdLockConnectionManager {
        RdLockConnectionManager::new("redis://localhost")
            .await
            .unwrap()
    }
}
