use bb8_redis::{bb8::Pool, RedisConnectionManager};

pub(crate) type RedisConnectionPool = Pool<RedisConnectionManager>;

pub(crate) fn get_lock_file(id: &String) -> String {
    format!("/rdlock/{id}/.lock")
}

pub(crate) enum LockState {
    Locked,
    Unlocked,
}

impl std::default::Default for LockState {
    fn default() -> Self {
        Self::Unlocked
    }
}

pub mod lock;
