use redis::{Commands, FromRedisValue, ToRedisArgs};
use crate::REDIS;

const EXP_TIME: u64 = 60 * 60; // 1 hour

pub(crate) fn cache_set(key: impl ToRedisArgs, value: impl ToRedisArgs) -> anyhow::Result<()> {
    if let Some(redis_cache) = REDIS.clone() {
        let mut con = redis_cache.get_connection()?;

        con.set_ex(key, value, EXP_TIME)?;
    }

    Ok(())
}

pub(crate) async fn cache_get<T: FromRedisValue>(key: impl ToRedisArgs) -> anyhow::Result<Option<T>> {
    if let Some(redis_cache) = REDIS.clone() {
        let mut con = redis_cache.get_connection()?;

        Ok(con.get(key)?)
    } else {
        Ok(None)
    }
}

pub(crate) async fn cache_del(key: impl ToRedisArgs) -> anyhow::Result<()> {
    if let Some(redis_cache) = REDIS.clone() {
        let mut con = redis_cache.get_connection()?;

        con.del(key)?;
    }

    Ok(())
}

pub(crate) async fn cache_clear() -> anyhow::Result<()> {
    if let Some(redis_cache) = REDIS.clone() {
        let mut con = redis_cache.get_connection()?;
        redis::cmd("FLUSHALL ASYNC").execute(&mut con);
    }

    Ok(())
}