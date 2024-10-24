use redis::{aio::ConnectionManager, cmd};

use crate::{errors::RdLockError, RdLockResult, DEFAULT_LOCK_VERSION};

/// Returns `(lock_file, data_file)`
pub(crate) fn get_redis_lock_location(id: &str) -> (String, String) {
    (format!("/rdlock/{id}/.lock"), format!("/rdlock/{id}/data"))
}

/// Does a redis SETNX type lock and checks the return data. Returns `Ok(())` on success
pub(crate) async fn redis_set_nx(
    mut conn: ConnectionManager,
    ttl: u64,
    lock_file: &str,
) -> RdLockResult {
    let res: Result<Option<String>, redis::RedisError> = cmd("SET")
        .arg(lock_file)
        .arg(DEFAULT_LOCK_VERSION.to_string())
        .arg("NX")
        .arg("EX")
        .arg(ttl)
        .query_async(&mut conn)
        .await;

    // Redis should return "OK" if operation success.
    // Should return `nil` when value already exists.
    match res.map_err(|_| RdLockError::LockUnavailable)? {
        Some(_) => Ok(()),
        _ => Err(RdLockError::LockUnavailable),
    }
}

/// Removes a redis file
/// Returns true if success or file didnt exist
async fn redis_remove_file(mut conn: ConnectionManager, file_name: &str) -> bool {
    let res: Result<i32, redis::RedisError> =
        cmd("DEL").arg(file_name).query_async(&mut conn).await;

    res.is_ok()
}

/// Removes a lock file without checking anything
pub(crate) async fn redis_remove_lockfile(conn: ConnectionManager, lock_file: &str) -> bool {
    redis_remove_file(conn, lock_file).await
}

/// Gets a redis configuration key
pub(crate) async fn redis_get_config(
    conn: &mut ConnectionManager,
    config_key: &str,
) -> Result<String, RdLockError> {
    Ok(cmd("CONFIG")
        .arg("GET")
        .arg(config_key)
        .query_async::<Vec<String>>(conn)
        .await?
        .last()
        .ok_or(RdLockError::InitCheckFailed)?
        .clone())
}

#[derive(Debug)]
pub(crate) enum LockVersion {
    V1,
}

pub(crate) enum LockVersionParseError {
    UnknownVersion,
}

impl std::fmt::Display for LockVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::str::FromStr for LockVersion {
    type Err = LockVersionParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "V1" => Ok(Self::V1),
            _ => Err(LockVersionParseError::UnknownVersion),
        }
    }
}

macro_rules! local_ttl_check {
    ($self:ident) => {
        if $self.is_locked && $self.lock_time.elapsed().as_secs() >= $self.ttl {
            $self.is_locked = false;
        }
    };
}
pub(crate) use local_ttl_check;
