use redis::{AsyncCommands, ExistenceCheck, SetExpiry, SetOptions, aio::ConnectionManager};
use serde::{Serialize, de::DeserializeOwned};
use std::time::Duration;

#[derive(Clone)]
pub struct Cache {
    connection: ConnectionManager,
}

impl Cache {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(url)?;
        Ok(Self {
            connection: ConnectionManager::new(client).await?,
        })
    }

    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let mut connection = self.connection.clone();
        let value: Option<String> = connection.get(key).await.ok()?;
        value.and_then(|raw| serde_json::from_str(&raw).ok())
    }

    pub async fn set_json<T: Serialize>(&self, key: &str, value: &T, ttl: Duration) {
        let Ok(value) = serde_json::to_string(value) else {
            return;
        };
        let mut connection = self.connection.clone();
        let _: redis::RedisResult<()> = connection.set_ex(key, value, ttl.as_secs()).await;
    }

    pub async fn delete(&self, key: &str) {
        let mut connection = self.connection.clone();
        let _: redis::RedisResult<()> = connection.del(key).await;
    }

    pub async fn take_lock(&self, key: &str, token: &str, ttl: Duration) -> bool {
        let mut connection = self.connection.clone();
        let options = SetOptions::default()
            .conditional_set(ExistenceCheck::NX)
            .with_expiration(SetExpiry::EX(ttl.as_secs()));
        connection
            .set_options::<_, _, Option<String>>(key, token, options)
            .await
            .ok()
            .flatten()
            .is_some()
    }

    pub async fn release_lock(&self, key: &str, token: &str) {
        // Only delete a lock owned by this request; never release another writer's lock.
        const RELEASE: &str = "if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('DEL', KEYS[1]) else return 0 end";
        let mut connection = self.connection.clone();
        let _: redis::RedisResult<i64> = redis::Script::new(RELEASE)
            .key(key)
            .arg(token)
            .invoke_async(&mut connection)
            .await;
    }

    pub async fn increment_with_ttl(&self, key: &str, ttl: Duration) -> Option<i64> {
        let mut connection = self.connection.clone();
        let count: i64 = connection.incr(key, 1).await.ok()?;
        if count == 1 {
            let _: redis::RedisResult<bool> = connection.expire(key, ttl.as_secs() as i64).await;
        }
        Some(count)
    }
}
