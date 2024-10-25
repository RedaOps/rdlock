use redis::RedisError;

/// Errors returned from various `rdlock` functions
#[derive(Debug, PartialEq)]
pub enum RdLockError {
    /// Error related to redis
    RedisError(RedisError),
    /// The lock is currently unavailable and could not be acquired
    LockUnavailable,
    /// Timeout has been reached
    TimeoutReached,
    /// The lock was not acquired by this instance
    LockNotAcquired,
    /// Connection error or error related to a redis command
    ConnectionError,
    /// The redis instance does not support distributed locks
    InitCheckFailed,
}

impl From<RedisError> for RdLockError {
    fn from(value: RedisError) -> Self {
        Self::RedisError(value)
    }
}

impl std::fmt::Display for RdLockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::RedisError(a) => format!("Redis error: {:?}", a),
            Self::LockUnavailable => "LockUnavailable".to_string(),
            Self::TimeoutReached => "TimeoutReached".to_string(),
            Self::LockNotAcquired => "LockNotAcquired: The lock is currently not acquired".to_string(),
            Self::ConnectionError => "ConnectionError".to_string(),
            Self::InitCheckFailed => "InitCheckFailed: redis does not support distributed locks. Check README.md and make sure notify-keyspace-events is set".to_string()
        };
        write!(f, "{msg}")
    }
}

impl std::error::Error for RdLockError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::RedisError(e) = self {
            Some(e)
        } else {
            None
        }
    }
}
