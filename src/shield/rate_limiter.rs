use crate::shield::{bucket::Bucket, config::RateLimiterConfig};
use dashmap::DashMap;
use std::{net::IpAddr, sync::Arc, time::Duration};

const CLEANUP_OLD_BUCKETS_SECS: u64 = 600; // 10 mins

pub struct RateLimiter {
    config: RateLimiterConfig,
    buckets: Arc<DashMap<IpAddr, Bucket>>,
}

impl RateLimiter {
    pub fn new(config: RateLimiterConfig) -> Self {
        let buckets = Arc::new(DashMap::new());

        let worker_buckets = Arc::clone(&buckets);

        Self::setup_cleanup_worker(worker_buckets);

        Self { config, buckets }
    }

    /// Sets up a cleanup worker that removes stale buckets from the map.
    fn setup_cleanup_worker(buckets: Arc<DashMap<IpAddr, Bucket>>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_mins(5)).await;

                buckets.retain(|_, bucket| {
                    bucket.refilled_at().elapsed().as_secs() < CLEANUP_OLD_BUCKETS_SECS
                });

                tracing::debug!("Cleared stale buckets from RateLimiter");
            }
        });
    }

    /// Attempt to consume tokens from the client bucket.
    /// Returns a tuple containing a boolean indicating if the client is rate-limited
    /// and the remaining tokens in the bucket.
    pub fn is_rate_limited(&self, client_ip: IpAddr) -> (bool, f64) {
        let mut client_bucket = self
            .buckets
            .entry(client_ip)
            .or_insert_with(|| Bucket::new(self.config.buckets_capacity));

        client_bucket.refill(
            self.config.buckets_capacity,
            self.config.refill_rate,
            self.config.refill_interval,
        );

        let is_rate_limited = !client_bucket.try_consume_token();

        (is_rate_limited, client_bucket.tokens())
    }

    pub fn buckets_capacity(&self) -> f64 {
        self.config.buckets_capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter() {
        tokio::time::pause();

        let config = RateLimiterConfig {
            buckets_capacity: 10.0,
            refill_rate: 1.0,
            refill_interval: 1.0,
        };

        let rate_limiter = RateLimiter::new(config);

        let (is_rate_limited, tokens) = rate_limiter.is_rate_limited(IpAddr::from([127, 0, 0, 1]));
        assert!(!is_rate_limited);
        assert_eq!(tokens, 9.0);

        let (is_rate_limited, tokens) = rate_limiter.is_rate_limited(IpAddr::from([127, 0, 0, 1]));
        assert!(!is_rate_limited);
        assert_eq!(tokens, 8.0);

        let (is_rate_limited, tokens) =
            rate_limiter.is_rate_limited(IpAddr::from([192, 168, 0, 1]));
        assert!(!is_rate_limited);
        assert_eq!(tokens, 9.0);
    }

    #[tokio::test]
    async fn test_rate_limiter_refill() {
        tokio::time::pause();

        let config = RateLimiterConfig {
            buckets_capacity: 10.0,
            refill_rate: 1.0,
            refill_interval: 1.0,
        };

        let rate_limiter = RateLimiter::new(config);

        let (is_rate_limited, tokens) = rate_limiter.is_rate_limited(IpAddr::from([127, 0, 0, 1]));
        assert!(!is_rate_limited);
        assert_eq!(tokens, 9.0);

        let (is_rate_limited, tokens) = rate_limiter.is_rate_limited(IpAddr::from([127, 0, 0, 1]));
        assert!(!is_rate_limited);
        assert_eq!(tokens, 8.0);

        tokio::time::advance(Duration::from_secs(2)).await;

        let (is_rate_limited, tokens) = rate_limiter.is_rate_limited(IpAddr::from([127, 0, 0, 1]));
        assert!(!is_rate_limited);
        assert_eq!(tokens, 9.0);
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks() {
        tokio::time::pause();
        let config = RateLimiterConfig {
            buckets_capacity: 2.0,
            refill_rate: 1.0,
            refill_interval: 1.0,
        };
        let rate_limiter = RateLimiter::new(config);

        let (is_limited, _) = rate_limiter.is_rate_limited(IpAddr::from([127, 0, 0, 1]));
        assert!(!is_limited);

        let (is_limited, _) = rate_limiter.is_rate_limited(IpAddr::from([127, 0, 0, 1]));
        assert!(!is_limited);

        let (is_limited, tokens) = rate_limiter.is_rate_limited(IpAddr::from([127, 0, 0, 1]));
        assert!(is_limited);
        assert_eq!(tokens, 0.0);
    }
}
