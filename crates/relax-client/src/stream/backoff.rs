//! 退避重连（手册 3.8）：1s → 2s → 4s → 8s → 16s → 30s（上限），成功后重置。
//! 鉴权失败（1001/1008）不重连，由上层直接终止。

/// 退避序列（毫秒）；到达 30s 后保持上限，不再增长。
pub const BACKOFF_MS: [u64; 6] = [1_000, 2_000, 4_000, 8_000, 16_000, 30_000];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backoff {
    attempt: usize,
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

impl Backoff {
    pub fn new() -> Self {
        Self { attempt: 0 }
    }

    /// 返回下一次重连延迟（毫秒），随后 attempt+1 至上限封顶。
    pub fn next_delay_ms(&mut self) -> u64 {
        let index = self.attempt.min(BACKOFF_MS.len() - 1);
        let delay = BACKOFF_MS[index];
        if self.attempt < BACKOFF_MS.len() - 1 {
            self.attempt += 1;
        }
        delay
    }

    /// 连接成功后重置退避。
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn attempt(&self) -> usize {
        self.attempt
    }
}

/// 终止码：鉴权失败不重连（手册 5.11：按 1001/1008 处理，不无限重连）。
pub fn is_fatal(code: u16) -> bool {
    code == 1001 || code == 1008
}
