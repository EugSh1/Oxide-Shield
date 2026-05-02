use std::time::Duration;
use tokio::time::Instant;

pub struct Bucket {
    tokens: f64,
    refilled_at: Instant,
}

impl Bucket {
    pub fn new(initial_tokens: f64) -> Self {
        Self {
            tokens: initial_tokens,
            refilled_at: Instant::now(),
        }
    }

    /// Add tokens based on elapsed time since the last refill.
    pub fn refill(&mut self, bucket_capacity: f64, refill_rate: f64, refill_interval: f64) {
        let elapsed = self.refilled_at.elapsed().as_secs_f64();

        if elapsed >= refill_interval {
            let refills = elapsed.div_euclid(refill_interval);

            if let Some(new_refilled_at) = self
                .refilled_at
                .checked_add(Duration::from_secs_f64(refills * refill_interval))
            {
                self.tokens = f64::min(bucket_capacity, self.tokens + refills * refill_rate);
                self.refilled_at = new_refilled_at;
            } else {
                self.refilled_at = Instant::now();
            }
        }
    }

    /// Attempt to consume tokens from the bucket and return if successful.
    /// Returns `true` if tokens were consumed, `false` otherwise.
    pub fn try_consume_token(&mut self) -> bool {
        let tokens_to_consume = 1.0;

        if self.tokens >= tokens_to_consume {
            self.tokens -= tokens_to_consume;
            true
        } else {
            false
        }
    }

    pub fn tokens(&self) -> f64 {
        self.tokens
    }

    pub fn refilled_at(&self) -> Instant {
        self.refilled_at
    }
}
