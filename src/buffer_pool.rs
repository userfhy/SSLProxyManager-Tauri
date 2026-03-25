//! Bytes 缓冲池 - 用于复用频繁分配的缓冲区，减少内存分配开销
//!
//! 使用对象池模式管理 BytesMut 缓冲区，避免频繁的内存分配和释放。
//! 适用于高并发场景下的请求/响应体处理。

use bytes::BytesMut;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::sync::Arc;

/// 缓冲区默认容量（64KB）
const DEFAULT_BUFFER_CAPACITY: usize = 64 * 1024;

/// 缓冲区最大容量（1MB）- 超过此大小的缓冲区不会被回收
const MAX_BUFFER_CAPACITY: usize = 1024 * 1024;

/// 池中最大缓冲区数量
const MAX_POOL_SIZE: usize = 256;

/// 全局缓冲池实例
pub static BUFFER_POOL: Lazy<BufferPool> = Lazy::new(|| BufferPool::new(MAX_POOL_SIZE));

/// 缓冲池结构
pub struct BufferPool {
    pool: Arc<Mutex<Vec<BytesMut>>>,
    #[allow(dead_code)]
    max_size: usize,
}

impl BufferPool {
    /// 创建新的缓冲池
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(Vec::with_capacity(max_size))),
            max_size,
        }
    }

    /// 从池中获取缓冲区，如果池为空则创建新的
    #[inline]
    pub fn acquire(&self) -> PooledBuffer {
        let buffer = {
            let mut pool = self.pool.lock();
            pool.pop()
                .unwrap_or_else(|| BytesMut::with_capacity(DEFAULT_BUFFER_CAPACITY))
        };

        PooledBuffer {
            buffer,
            detached: false,
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
        }
    }

    /// 将缓冲区归还到池中
    #[allow(dead_code)]
    #[inline]
    fn release(&self, mut buffer: BytesMut) {
        // 只回收容量合理的缓冲区
        if buffer.capacity() <= MAX_BUFFER_CAPACITY {
            buffer.clear(); // 清空数据但保留容量

            let mut pool = self.pool.lock();
            if pool.len() < self.max_size {
                pool.push(buffer);
            }
            // 如果池已满，缓冲区会被丢弃（自动释放内存）
        }
        // 超大缓冲区直接丢弃，避免占用过多内存
    }

    /// 获取池中当前缓冲区数量
    #[inline]
    pub fn size(&self) -> usize {
        self.pool.lock().len()
    }

    /// 清空池中所有缓冲区
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.pool.lock().clear();
    }
}

/// 池化的缓冲区 - 实现 RAII 模式，自动归还到池中
pub struct PooledBuffer {
    buffer: BytesMut,
    detached: bool,
    pool: Arc<Mutex<Vec<BytesMut>>>,
    max_size: usize,
}

impl PooledBuffer {
    /// 获取内部缓冲区的可变引用
    #[inline]
    pub fn get_mut(&mut self) -> &mut BytesMut {
        &mut self.buffer
    }

    /// 获取内部缓冲区的不可变引用
    #[inline]
    pub fn get(&self) -> &BytesMut {
        &self.buffer
    }

    /// 取出内部缓冲区（不归还到池中）
    #[allow(dead_code)]
    #[inline]
    pub fn take(mut self) -> BytesMut {
        self.detached = true;
        std::mem::take(&mut self.buffer)
    }

    /// 转换为 Bytes（冻结缓冲区）
    #[allow(dead_code)]
    #[inline]
    pub fn freeze(mut self) -> bytes::Bytes {
        self.detached = true;
        std::mem::take(&mut self.buffer).freeze()
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if self.detached {
            return;
        }

        // 归还到池中
        let mut pool = self.pool.lock();
        if self.buffer.capacity() <= MAX_BUFFER_CAPACITY && pool.len() < self.max_size {
            self.buffer.clear();
            pool.push(std::mem::take(&mut self.buffer));
        }
    }
}

impl std::ops::Deref for PooledBuffer {
    type Target = BytesMut;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl std::ops::DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

/// 便捷函数：从池中获取缓冲区
#[inline]
pub fn acquire_buffer() -> PooledBuffer {
    BUFFER_POOL.acquire()
}

/// 便捷函数：获取池的统计信息
#[inline]
pub fn pool_stats() -> PoolStats {
    PoolStats {
        size: BUFFER_POOL.size(),
        max_size: MAX_POOL_SIZE,
    }
}

/// 缓冲池统计信息
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub size: usize,
    pub max_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_acquire_release() {
        let pool = BufferPool::new(10);

        // 获取缓冲区
        let mut buf = pool.acquire();
        assert_eq!(buf.len(), 0);
        assert!(buf.capacity() >= DEFAULT_BUFFER_CAPACITY);

        // 写入数据
        buf.extend_from_slice(b"hello world");
        assert_eq!(buf.len(), 11);

        // 释放缓冲区（通过 drop）
        drop(buf);

        // 池中应该有 1 个缓冲区
        assert_eq!(pool.size(), 1);

        // 再次获取应该复用之前的缓冲区
        let buf2 = pool.acquire();
        assert_eq!(buf2.len(), 0); // 已清空
        assert!(buf2.capacity() >= DEFAULT_BUFFER_CAPACITY);
    }

    #[test]
    fn test_buffer_pool_max_size() {
        let pool = BufferPool::new(2);

        // 获取 3 个缓冲区
        let buf1 = pool.acquire();
        let buf2 = pool.acquire();
        let buf3 = pool.acquire();

        // 释放所有缓冲区
        drop(buf1);
        drop(buf2);
        drop(buf3);

        // 池中最多只有 2 个缓冲区
        assert_eq!(pool.size(), 2);
    }

    #[test]
    fn test_buffer_pool_oversized() {
        let pool = BufferPool::new(10);

        // 获取缓冲区并扩展到超大容量
        let mut buf = pool.acquire();
        buf.reserve(MAX_BUFFER_CAPACITY + 1024);

        // 释放缓冲区
        drop(buf);

        // 超大缓冲区不应该被回收
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn test_pooled_buffer_freeze() {
        let pool = BufferPool::new(10);

        let mut buf = pool.acquire();
        buf.extend_from_slice(b"test data");

        let bytes = buf.freeze();
        assert_eq!(&bytes[..], b"test data");

        // freeze 后缓冲区不会归还到池中
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn test_acquire_buffer_convenience() {
        let mut buf = acquire_buffer();
        buf.extend_from_slice(b"test");
        assert_eq!(&buf[..], b"test");
    }

    #[test]
    fn test_pool_stats() {
        let stats = pool_stats();
        assert_eq!(stats.max_size, MAX_POOL_SIZE);
    }
}
