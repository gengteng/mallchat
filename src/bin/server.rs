mod service {
    use anyhow::Context;
    use mallchat::cache::CacheConfig;
    use mallchat::handler::auth::JwtKeys;
    use mallchat::handler::HttpConfig;
    use mallchat::log::LogConfig;
    use mallchat::storage::StorageConfig;
    use mallchat::weixin::{WxClient, WxConfig};
    use serde::{Deserialize, Serialize};
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use time::UtcOffset;

    #[derive(Debug, Serialize, Deserialize)]
    struct Config {
        http: HttpConfig,
        wx: WxConfig,
        storage: StorageConfig,
        cache: CacheConfig,
        log: LogConfig,
    }

    #[tokio::main]
    async fn tokio_start(config: Config, offset: UtcOffset) -> anyhow::Result<()> {
        let Config {
            http,
            wx,
            storage,
            cache,
            log,
        } = config;

        let _logger = log.init("mallchat", ".", offset, true).await?;

        tracing::info!(?storage, "Connect to database.");
        let storage = storage.connect().await?;

        tracing::info!(?cache, "Connect to redis.");
        let cache = cache.connect().await?;

        let key = JwtKeys::try_from(http.jwt_secret.as_str())?;
        let wx_client = WxClient::new(wx).await?;
        tracing::info!(app_id = %wx_client.app_id(), "Retrieve weixin acccess token.");

        let addr = SocketAddr::from(([0, 0, 0, 0], http.port));
        tracing::info!(%addr, "Server start.");

        let router =
            mallchat::handler::router(true, http.static_files_path, storage, cache, key, wx_client);
        axum::Server::bind(&addr)
            .serve(router.into_make_service_with_connect_info::<SocketAddr>())
            .await?;
        Ok(())
    }

    pub(crate) fn start() -> anyhow::Result<()> {
        let path = PathBuf::from("server.toml");
        let offset = UtcOffset::current_local_offset()?;

        let config = config::Config::builder()
            .add_source(config::File::from(path.as_path()))
            .add_source(config::Environment::with_prefix("MALLCHAT").separator("__"))
            .build()
            .context("read config")?
            .try_deserialize()
            .context("deserialize config")?;

        tokio_start(config, offset)
    }
}

fn main() -> anyhow::Result<()> {
    service::start()
}
