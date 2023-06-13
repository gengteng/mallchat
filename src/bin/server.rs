#[cfg(not(feature = "shuttle"))]
mod local {
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
            cache: _,
            log,
        } = config;

        let _logger = log.init("mallchat", ".", offset, true).await?;

        tracing::info!(?storage, "Connect to database.");
        // TODO use this storage
        let _storage = storage.connect().await?;

        // TODO use this cache
        // tracing::info!(?cache, "Connect to redis.");
        // TODO use this cache
        //let _cache = cache.connect().await?;

        let key = JwtKeys::try_from(http.jwt_secret.as_str())?;
        let wx_client = WxClient::new(wx).await?;
        tracing::info!(app_id = %wx_client.app_id(), "Retrieve weixin acccess token.");

        let addr = SocketAddr::from(([0, 0, 0, 0], http.port));
        tracing::info!(%addr, "Server start.");

        let router = mallchat::handler::router(true, http.static_files_path, key, wx_client);
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

#[cfg(not(feature = "shuttle"))]
fn main() -> anyhow::Result<()> {
    local::start()
}

#[cfg(feature = "shuttle")]
mod shuttle {
    use mallchat::handler::auth::JwtKeys;
    use mallchat::weixin::WxClient;
    use shuttle_runtime::async_trait;
    use shuttle_service::Error;
    use std::net::SocketAddr;
    use std::path::PathBuf;

    pub struct MallChatService {
        pub(crate) with_swagger: bool,
        pub(crate) static_files_path: PathBuf,
        pub(crate) jwt_keys: JwtKeys,
        pub(crate) wx_client: WxClient,
    }

    #[async_trait]
    impl shuttle_service::Service for MallChatService {
        async fn bind(self, addr: SocketAddr) -> Result<(), Error> {
            let router = mallchat::handler::router(
                self.with_swagger,
                self.static_files_path,
                self.jwt_keys,
                self.wx_client,
            );

            axum::Server::bind(&addr)
                .serve(router.into_make_service_with_connect_info::<SocketAddr>())
                .await
                .map_err(|e| Error::Custom(e.into()))?;
            Ok(())
        }
    }
}

#[cfg(feature = "shuttle")]
#[shuttle_runtime::main]
async fn shuttle_main(
    #[shuttle_static_folder::StaticFolder(folder = "html")] static_files_path: std::path::PathBuf,
    #[shuttle_secrets::Secrets] secret_store: shuttle_secrets::SecretStore,
) -> Result<shuttle::MallChatService, shuttle_runtime::Error> {
    use mallchat::*;
    let jwt_secret = secret_store
        .get("JWT_SECRET")
        .ok_or_else(|| anyhow::anyhow!("Failed to get JWT_SECRET from secret store"))?;
    let jwt_keys = handler::auth::JwtKeys::try_from(jwt_secret.as_str())
        .map_err(|e| anyhow::anyhow!("Failed to build jwt keys: {e}"))?;
    let encoding_aes_key = secret_store
        .get("WX_ENCODING_AES_KEY")
        .ok_or_else(|| anyhow::anyhow!("Failed to get WX_ENCODING_AES_KEY from secret store"))?;
    let wx_config = weixin::WxConfig {
        app_id: secret_store
            .get("WX_APP_ID")
            .ok_or_else(|| anyhow::anyhow!("Failed to get WX_APP_ID from secret store"))?,
        app_secret: secret_store
            .get("WX_APP_SECRET")
            .ok_or_else(|| anyhow::anyhow!("Failed to get WX_APP_SECRET from secret store"))?,
        token: secret_store
            .get("WX_TOKEN")
            .ok_or_else(|| anyhow::anyhow!("Failed to get WX_TOKEN from secret store"))?,
        encoding_aes_key: encoding_aes_key.parse()?,
        timeout_secs: secret_store
            .get("WX_TIMEOUT_SECS")
            .unwrap_or_else(|| String::from("10"))
            .parse::<u64>()
            .map_err(|e| shuttle_runtime::Error::Custom(e.into()))?,
    };

    let wx_client = weixin::WxClient::new(wx_config).await?;

    let service = shuttle::MallChatService {
        with_swagger: false,
        static_files_path,
        jwt_keys,
        wx_client,
    };
    Ok(service)
}
