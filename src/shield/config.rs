use anyhow::{Context, Result, anyhow, bail};
use hyper::Uri;
use std::net::SocketAddr;

/// Parses an environment variable as a given type.
macro_rules! parse_env {
    ($key:expr, $type:ty) => {
        std::env::var($key)
            .context(format!(
                "Environment variable {} is missing or contains invalid unicode",
                $key
            ))?
            .parse::<$type>()
            .context(format!("Failed to parse {} as {}", $key, stringify!($type)))?
    };

    ($key:expr) => {
        std::env::var($key).context(format!(
            "Environment variable {} is missing or contains invalid unicode",
            $key
        ))
    };

    ($key:expr => optional) => {
        std::env::var($key)
            .context(format!(
                "Environment variable {} is missing or contains invalid unicode",
                $key
            ))
            .ok()
    };
}

const BIND_ADDR_ENV_VAR_NAME: &str = "OXIDE_SHIELD_BIND_ADDR";
const SERVER_ADDR_ENV_VAR_NAME: &str = "OXIDE_SHIELD_SERVER_URI";
const IP_HEADER_NAME_ENV_VAR_NAME: &str = "OXIDE_SHIELD_IP_HEADER_NAME";
const BUCKETS_CAPACITY_ENV_VAR_NAME: &str = "OXIDE_SHIELD_BUCKETS_CAPACITY";
const REFILL_RATE_ENV_VAR_NAME: &str = "OXIDE_SHIELD_REFILL_RATE";
const REFILL_INTERVAL_ENV_VAR_NAME: &str = "OXIDE_SHIELD_REFILL_INTERVAL";

pub struct Config {
    pub server: ServerConfig,
    pub rate_limiter: RateLimiterConfig,
}

pub struct ServerConfig {
    /// The address to bind the server to.
    pub bind_addr: SocketAddr,

    /// The address of the server to proxy requests to.
    pub server_addr: String,

    /// Optional IP header name to use for rate-limiting.
    /// Should be used when the rate-limiter is behind a reverse proxy.
    /// If not specified, the IP address of the client will be used.
    pub ip_header_name: Option<String>,
}

pub struct RateLimiterConfig {
    /// The maximum amount of tokens a bucket can hold.
    /// This defines how many requests a client can send at once
    /// before being rate-limited.
    pub buckets_capacity: f64,

    /// How many tokens are added to the bucket per `refill_interval`.
    pub refill_rate: f64,

    /// Time interval (in seconds) at which tokens are refilled.
    pub refill_interval: f64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            server: ServerConfig {
                bind_addr: parse_env!(BIND_ADDR_ENV_VAR_NAME, SocketAddr),
                server_addr: Self::parse_server_addr(&parse_env!(SERVER_ADDR_ENV_VAR_NAME)?)?,
                ip_header_name: parse_env!(IP_HEADER_NAME_ENV_VAR_NAME => optional),
            },
            rate_limiter: RateLimiterConfig {
                buckets_capacity: parse_env!(BUCKETS_CAPACITY_ENV_VAR_NAME, f64),
                refill_rate: parse_env!(REFILL_RATE_ENV_VAR_NAME, f64),
                refill_interval: parse_env!(REFILL_INTERVAL_ENV_VAR_NAME, f64),
            },
        })
    }

    fn parse_server_addr(server_addr_str: &str) -> Result<String> {
        let server_addr_uri = server_addr_str.parse::<Uri>().context(format!(
            "Invalid uri format in {SERVER_ADDR_ENV_VAR_NAME}='{server_addr_str}'"
        ))?;

        let authority = server_addr_uri.authority().ok_or_else(|| {
            anyhow!("URI must contain host: {SERVER_ADDR_ENV_VAR_NAME}='{server_addr_str}'")
        })?;

        if let Some(port) = authority.port() {
            if port.as_u16() == 443
                || server_addr_uri
                    .scheme_str()
                    .is_some_and(|scheme| scheme == "https")
            {
                tracing::warn!("Oxide-Shield does not support HTTPS");
            }
        } else {
            bail!("URI must include a port: {SERVER_ADDR_ENV_VAR_NAME}='{server_addr_str}'");
        }

        Ok(authority.to_string())
    }
}
