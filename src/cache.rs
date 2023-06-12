//! # 外部缓存

use redis::{ConnectionAddr, ConnectionInfo, RedisConnectionInfo};
use serde::{Deserialize, Serialize};

/// 外部 Redis 缓存配置
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 主机
    pub host: String,
    /// 端口
    pub port: u16,
    /// 密码
    pub password: String,
}

impl CacheConfig {
    /// 连接 redis 数据库
    pub async fn connect(self) -> anyhow::Result<redis::Client> {
        let opts = ConnectionInfo {
            addr: ConnectionAddr::Tcp(self.host, self.port),
            redis: RedisConnectionInfo {
                db: 0,
                username: None,
                password: Some(self.password),
            },
        };
        let client = redis::Client::open(opts)?;
        client.get_async_connection().await?;
        Ok(client)
    }
}
