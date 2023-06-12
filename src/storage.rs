//! # 持久化存储
//!
//! 使用如下命令可以生成 MySQL 中表对应的 ORM 模型：
//!
//! ```shell
//! sea generate entity -u mysql://root:123456@localhost:3306/mallchat --date-time-crate time --with-serde both -o src/storage/model
//! ```
//!

use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[allow(missing_docs)]
pub mod model;

/// 数据库配置
#[derive(Debug, Serialize, Deserialize)]
pub struct StorageConfig {
    /// 主机
    pub host: String,
    /// 端口
    pub port: u16,
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
    /// 数据库
    pub database: String,
}

impl StorageConfig {
    /// 构造连接字符串
    pub fn url(&self) -> String {
        format!(
            "mysql://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.database
        )
    }
    /// 构造数据库连接
    pub async fn connect(self) -> anyhow::Result<DatabaseConnection> {
        let mut opts: ConnectOptions = self.url().into();
        opts.connect_timeout(Duration::from_secs(10));
        Ok(Database::connect(opts).await?)
    }
}
