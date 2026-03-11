// 缓存优化模块
// 提供正则表达式缓存、DNS 缓存等功能

use anyhow::Result;
use lru::LruCache;
use parking_lot::Mutex;
use regex::Regex;
use std::net::IpAddr;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::debug;

/// 正则表达式缓存
pub struct RegexCache {
    cache: Mutex<LruCache<String, Arc<Regex>>>,
}

impl RegexCache {
    /// 创建新的正则表达式缓存
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(100).unwrap()),
            )),
        }
    }

    /// 获取或编译正则表达式
    pub fn get_or_compile(&self, pattern: &str) -> Result<Arc<Regex>> {
        // 先尝试从缓存获取
        {
            let mut cache = self.cache.lock();
            if let Some(regex) = cache.get(pattern) {
                debug!("Regex cache hit: {}", pattern);
                return Ok(Arc::clone(regex));
            }
        }

        // 缓存未命中，编译新的正则表达式
        debug!("Regex cache miss, compiling: {}", pattern);
        let regex = Regex::new(pattern)?;
        let regex_arc = Arc::new(regex);

        // 存入缓存
        {
            let mut cache = self.cache.lock();
            cache.put(pattern.to_string(), Arc::clone(&regex_arc));
        }

        Ok(regex_arc)
    }

    /// 清空缓存
    pub fn clear(&self) {
        let mut cache = self.cache.lock();
        cache.clear();
        debug!("Regex cache cleared");
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.lock();
        CacheStats {
            size: cache.len(),
            capacity: cache.cap().get(),
        }
    }
}

/// DNS 缓存
pub struct DnsCache {
    cache: Mutex<LruCache<String, Vec<IpAddr>>>,
}

impl DnsCache {
    /// 创建新的 DNS 缓存
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1000).unwrap()),
            )),
        }
    }

    /// 获取缓存的 DNS 记录
    pub fn get(&self, hostname: &str) -> Option<Vec<IpAddr>> {
        let mut cache = self.cache.lock();
        cache.get(hostname).cloned()
    }

    /// 存储 DNS 记录
    pub fn put(&self, hostname: String, ips: Vec<IpAddr>) {
        let mut cache = self.cache.lock();
        cache.put(hostname, ips);
    }

    /// 清空缓存
    pub fn clear(&self) {
        let mut cache = self.cache.lock();
        cache.clear();
        debug!("DNS cache cleared");
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.lock();
        CacheStats {
            size: cache.len(),
            capacity: cache.cap().get(),
        }
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub capacity: usize,
}

impl CacheStats {
    /// 计算缓存使用率
    pub fn usage_percent(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.size as f64 / self.capacity as f64) * 100.0
        }
    }
}

/// 全局缓存管理器
pub struct CacheManager {
    regex_cache: Arc<RegexCache>,
    dns_cache: Arc<DnsCache>,
}

impl CacheManager {
    /// 创建新的缓存管理器
    pub fn new() -> Self {
        Self {
            regex_cache: Arc::new(RegexCache::new(100)),
            dns_cache: Arc::new(DnsCache::new(1000)),
        }
    }

    /// 获取正则表达式缓存
    pub fn regex_cache(&self) -> Arc<RegexCache> {
        Arc::clone(&self.regex_cache)
    }

    /// 获取 DNS 缓存
    pub fn dns_cache(&self) -> Arc<DnsCache> {
        Arc::clone(&self.dns_cache)
    }

    /// 清空所有缓存
    pub fn clear_all(&self) {
        self.regex_cache.clear();
        self.dns_cache.clear();
        debug!("All caches cleared");
    }

    /// 获取所有缓存统计信息
    pub fn all_stats(&self) -> AllCacheStats {
        AllCacheStats {
            regex: self.regex_cache.stats(),
            dns: self.dns_cache.stats(),
        }
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 所有缓存的统计信息
#[derive(Debug, Clone)]
pub struct AllCacheStats {
    pub regex: CacheStats,
    pub dns: CacheStats,
}

/// 全局缓存管理器实例
static CACHE_MANAGER: once_cell::sync::Lazy<CacheManager> =
    once_cell::sync::Lazy::new(CacheManager::new);

/// 获取全局缓存管理器
pub fn global_cache_manager() -> &'static CacheManager {
    &CACHE_MANAGER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_cache() {
        let cache = RegexCache::new(10);

        // 第一次编译
        let regex1 = cache.get_or_compile(r"\d+").unwrap();
        assert!(regex1.is_match("123"));

        // 第二次应该从缓存获取
        let regex2 = cache.get_or_compile(r"\d+").unwrap();
        assert!(Arc::ptr_eq(&regex1, &regex2));

        // 统计信息
        let stats = cache.stats();
        assert_eq!(stats.size, 1);
        assert_eq!(stats.capacity, 10);
    }

    #[test]
    fn test_dns_cache() {
        let cache = DnsCache::new(100);

        // 存储 DNS 记录
        let ips = vec!["127.0.0.1".parse().unwrap()];
        cache.put("localhost".to_string(), ips.clone());

        // 获取 DNS 记录
        let cached = cache.get("localhost");
        assert_eq!(cached, Some(ips));

        // 统计信息
        let stats = cache.stats();
        assert_eq!(stats.size, 1);
    }

    #[test]
    fn test_cache_manager() {
        let manager = CacheManager::new();

        // 测试正则缓存
        let regex_cache = manager.regex_cache();
        let _ = regex_cache.get_or_compile(r"test").unwrap();

        // 测试 DNS 缓存
        let dns_cache = manager.dns_cache();
        dns_cache.put("example.com".to_string(), vec!["1.2.3.4".parse().unwrap()]);

        // 获取统计信息
        let stats = manager.all_stats();
        assert_eq!(stats.regex.size, 1);
        assert_eq!(stats.dns.size, 1);

        // 清空所有缓存
        manager.clear_all();
        let stats = manager.all_stats();
        assert_eq!(stats.regex.size, 0);
        assert_eq!(stats.dns.size, 0);
    }

    #[test]
    fn test_cache_stats() {
        let stats = CacheStats {
            size: 50,
            capacity: 100,
        };
        assert_eq!(stats.usage_percent(), 50.0);
    }
}
