use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 速率限制配置
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 是否启用速率限制
    pub enabled: bool,
    /// 每个IP的请求限制（每秒）
    pub requests_per_second: u32,
    /// 每个IP的突发请求数（令牌桶容量）
    pub burst_size: u32,
    /// 超过限制后封禁的秒数（0表示不封禁，只返回429）
    pub ban_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            requests_per_second: 10,
            burst_size: 20,
            ban_seconds: 0,
        }
    }
}

/// 令牌桶结构
struct TokenBucket {
    /// 当前令牌数
    tokens: f64,
    /// 令牌桶容量
    capacity: f64,
    /// 令牌补充速率（每秒）
    refill_rate: f64,
    /// 上次更新时间
    last_update: Instant,
}

impl TokenBucket {
    fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_rate,
            last_update: Instant::now(),
        }
    }

    /// 尝试消费一个令牌
    fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);
        
        // 补充令牌
        let tokens_to_add = elapsed.as_secs_f64() * self.refill_rate;
        self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
        self.last_update = now;

        // 尝试消费一个令牌
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

}

/// 速率限制器
pub struct RateLimiter {
    /// IP -> 令牌桶的映射
    buckets: Arc<DashMap<String, Arc<RwLock<TokenBucket>>>>,
    /// 配置
    config: RateLimitConfig,
    /// 清理任务句柄
    _cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        let buckets: Arc<DashMap<String, Arc<RwLock<TokenBucket>>>> = Arc::new(DashMap::new());
        let buckets_clone = buckets.clone();
        
        // 启动清理任务：定期清理长时间未使用的令牌桶
        let cleanup_handle = if config.enabled {
            Some(tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(300)); // 每5分钟清理一次
                loop {
                    interval.tick().await;
                    let now = Instant::now();
                    let keys_to_remove: Vec<String> = buckets_clone
                        .iter()
                        .filter_map(|entry| {
                            let bucket = entry.value().read();
                            if now.duration_since(bucket.last_update) >= Duration::from_secs(600) {
                                Some(entry.key().clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    for key in keys_to_remove {
                        buckets_clone.remove(&key);
                    }
                }
            }))
        } else {
            None
        };

        Self {
            buckets,
            config,
            _cleanup_handle: cleanup_handle,
        }
    }

    /// 检查是否允许请求，返回 (是否允许, 是否需要封禁)
    pub fn check(&self, ip: &str) -> (bool, bool) {
        if !self.config.enabled {
            return (true, false);
        }

        let bucket = self.buckets
            .entry(ip.to_string())
            .or_insert_with(|| {
                Arc::new(RwLock::new(TokenBucket::new(
                    self.config.burst_size as f64,
                    self.config.requests_per_second as f64,
                )))
            })
            .clone();

        let mut bucket = bucket.write();
        let allowed = bucket.try_consume();
        
        // 如果超过限制且配置了封禁时间，则标记需要封禁
        let should_ban = !allowed && self.config.ban_seconds > 0;
        
        (allowed, should_ban)
    }

}

/// 全局速率限制器（按监听地址分组）
pub static RATE_LIMITERS: once_cell::sync::Lazy<Arc<DashMap<String, Arc<RwLock<RateLimiter>>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(DashMap::new()));

/// 获取或创建速率限制器
pub fn get_rate_limiter(listen_addr: &str, config: RateLimitConfig) -> Arc<RwLock<RateLimiter>> {
    RATE_LIMITERS
        .entry(listen_addr.to_string())
        .or_insert_with(|| {
            Arc::new(RwLock::new(RateLimiter::new(config)))
        })
        .clone()
}

