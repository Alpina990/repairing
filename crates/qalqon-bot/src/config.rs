use std::{collections::HashSet, env, net::SocketAddr};

use anyhow::{Context, Result, bail};
use url::Url;

#[derive(Debug, Clone)]
pub struct Config {
    pub token: String,
    pub telegram_api_url: Option<Url>,
    pub database_url: String,
    pub database_max_connections: u32,
    pub owner_ids: HashSet<u64>,
    pub admin_cache_ttl_secs: u64,
    pub policy_cache_ttl_secs: u64,
    pub database_connect_max_attempts: u32,
    pub database_connect_backoff_ms: u64,
    pub health_addr: SocketAddr,
    pub api_addr: SocketAddr,
    pub mini_app_auth_max_age_secs: u64,
    pub mini_app_origin: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let token = env::var("TELOXIDE_TOKEN").context("TELOXIDE_TOKEN kiritilmagan")?;
        let database_url = env::var("DATABASE_URL").context("DATABASE_URL kiritilmagan")?;
        let database_max_connections = parse_env("DATABASE_MAX_CONNECTIONS", 10)?;
        if database_max_connections == 0 {
            bail!("DATABASE_MAX_CONNECTIONS 0 dan katta bo'lishi kerak");
        }

        let owner_ids = env::var("OWNER_IDS")
            .unwrap_or_default()
            .split(',')
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                value
                    .trim()
                    .parse::<u64>()
                    .with_context(|| format!("OWNER_IDS ichida xato ID: {value}"))
            })
            .collect::<Result<HashSet<_>>>()?;

        Ok(Self {
            token,
            telegram_api_url: env::var("TELEGRAM_API_URL")
                .ok()
                .map(|value| Url::parse(&value).context("TELEGRAM_API_URL noto'g'ri"))
                .transpose()?,
            database_url,
            database_max_connections,
            owner_ids,
            admin_cache_ttl_secs: parse_env("ADMIN_CACHE_TTL_SECS", 300)?,
            policy_cache_ttl_secs: parse_env("POLICY_CACHE_TTL_SECS", 30)?,
            database_connect_max_attempts: parse_env("DATABASE_CONNECT_MAX_ATTEMPTS", 10)?,
            database_connect_backoff_ms: parse_env("DATABASE_CONNECT_BACKOFF_MS", 500)?,
            health_addr: Self::health_addr_from_env()?,
            api_addr: parse_env("API_ADDR", "0.0.0.0:8081".parse()?)?,
            mini_app_auth_max_age_secs: parse_env("MINI_APP_AUTH_MAX_AGE_SECS", 3600)?,
            mini_app_origin: env::var("MINI_APP_ORIGIN")
                .ok()
                .filter(|value| !value.trim().is_empty()),
        })
    }

    pub fn health_addr_from_env() -> Result<SocketAddr> {
        parse_env("HEALTH_ADDR", "0.0.0.0:8080".parse()?)
    }
}

fn parse_env<T>(name: &str, default: T) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    env::var(name).map_or(Ok(default), |value| {
        value
            .parse::<T>()
            .with_context(|| format!("{name} qiymati noto'g'ri"))
    })
}
