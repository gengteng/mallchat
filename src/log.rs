#![cfg(not(feature = "shuttle"))]
//! # 日志
//!

use std::path::{Path, PathBuf};

use byte_unit::Byte;
use rolling_file::RollingConditionBasic;
use serde::{Deserialize, Serialize};
use time::format_description::FormatItem;
use time::UtcOffset;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::time::OffsetTime;
use tracing_subscriber::prelude::*;

/// # 日志时间格式
///
/// 使用 `2022-01-01 01:01:01.010` 的时间格式
const LOG_FORMAT: &[FormatItem] = time::macros::format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"
);

/// # 日志配置
#[derive(Debug, Deserialize, Serialize)]
pub struct LogConfig {
    /// 日志级别
    #[serde(with = "serde_level", default = "default::level")]
    pub level: tracing::Level,
    /// 文件保存地址
    #[serde(default = "default::path")]
    pub path: PathBuf,
    /// 文件大小
    #[serde(default = "default::trigger_size")]
    pub trigger_size: Byte,
    /// 文件个数
    #[serde(default = "default::archived_count")]
    pub archived_count: usize,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default::level(),
            path: default::path(),
            trigger_size: default::trigger_size(),
            archived_count: default::archived_count(),
        }
    }
}

mod default {
    use std::path::PathBuf;

    use byte_unit::Byte;

    pub fn level() -> tracing::Level {
        tracing::Level::INFO
    }

    pub fn path() -> PathBuf {
        PathBuf::from("log")
    }

    pub fn trigger_size() -> Byte {
        Byte::from_bytes(1024 * 1024)
    }

    pub fn archived_count() -> usize {
        32
    }
}

mod serde_level {
    use serde::{Deserialize, Deserializer, Serializer};

    // The signature of a serialize_with function must follow the pattern:
    //
    //    fn serialize<S>(&T, S) -> Result<S::Ok, S::Error>
    //    where
    //        S: Serializer
    //
    // although it may also be generic over the input types T.
    pub fn serialize<S>(level: &tracing::Level, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", level);
        serializer.serialize_str(&s)
    }

    // The signature of a deserialize_with function must follow the pattern:
    //
    //    fn deserialize<'de, D>(D) -> Result<T, D::Error>
    //    where
    //        D: Deserializer<'de>
    //
    // although it may also be generic over the output types T.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<tracing::Level, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// # 跟踪日志句柄
///
/// 在需要使用 tracing 期间需要保证其存活
#[must_use]
pub struct Logger {
    _guard: WorkerGuard,
}

impl LogConfig {
    /// 初始化，确保全局执行一次
    pub async fn init<P: AsRef<Path>>(
        self,
        service: &str,
        root_path: P,
        offset: UtcOffset,
        stdout: bool,
    ) -> anyhow::Result<Logger> {
        let local_time = OffsetTime::new(offset, LOG_FORMAT);

        let log_path = root_path.as_ref().join(&self.path);
        tokio::fs::create_dir_all(&log_path).await?;

        let file_appender = rolling_file::BasicRollingFileAppender::new(
            log_path.join(format!("{}.log", service)),
            RollingConditionBasic::new().max_size(self.trigger_size.get_bytes()),
            self.archived_count,
        )?;
        let (nonblocking, _guard) = tracing_appender::non_blocking(file_appender);

        if stdout {
            let registry = tracing_subscriber::Registry::default();

            let file_layer = tracing_subscriber::fmt::layer()
                .with_writer(nonblocking.with_max_level(self.level))
                .with_file(true)
                .with_line_number(true)
                .with_target(false)
                .with_timer(local_time.clone());

            let stdout_layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stdout.with_max_level(self.level))
                .with_file(true)
                .with_line_number(true)
                .with_target(false)
                .with_timer(local_time);

            let registry = registry.with(stdout_layer).with(file_layer);

            tracing::subscriber::set_global_default(registry)?;
        } else {
            let registry = tracing_subscriber::Registry::default();

            let file_layer = tracing_subscriber::fmt::layer()
                .with_writer(nonblocking.with_max_level(self.level))
                .with_ansi(false)
                .with_file(true)
                .with_line_number(true)
                .with_target(false)
                .with_timer(local_time.clone());

            let registry = registry.with(file_layer);

            tracing::subscriber::set_global_default(registry)?;
        }

        tracing::info!(log = ?self, "Global logger initialized.");

        Ok(Logger { _guard })
    }
}
