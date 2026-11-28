//! 文件传输（C4.5，手册 3.7 / 6.4）：上传/下载/断点续传/回收站客户端纯逻辑。
//!
//! - 上传：init 拿到 uploadId + chunkSize + receivedChunks，只传缺失片；中断后重启重新 init，服务端 received_chunks_json 已持久化 → 继续。
//! - 每片 X-Chunk-Sha256 头用 sha256 计算；complete 由服务端重算整文件 sha256（不信任客户端声明）。
//! - 下载：支持 Range 断点续传（已下字节数 → 下一段 bytes=start-end）。
//! - 回收站：软删除 30 天后过期（手册 4.7 recycle.retentionDays=30）。
//! 全部纯函数/内存结构，无 IO，便于离线单测。

use sha2::{Digest, Sha256};
use thiserror::Error;

/// 分片默认大小（手册 3.7：2 MiB）。
pub const DEFAULT_CHUNK_SIZE: i64 = 2 * 1024 * 1024;
/// 回收站保留天数（手册 4.7）。
pub const RECYCLE_RETENTION_DAYS: i64 = 30;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum TransferError {
    #[error("参数非法")]
    InvalidParam,
    #[error("契约不符：分片大小与服务端不一致")]
    ContractMismatch,
    #[error("上传中断（3005），续传")]
    Interrupted,
}

// ---- 上传：init 响应与续传会话 ----

/// init 响应（对齐 file-transfer.schema.json initResponse）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitResponse {
    pub upload_id: String,
    pub chunk_size: i64,
    pub received_chunks: Vec<u32>,
    pub expires_at: String,
}

/// 上传会话：本地记录 upload_id + 已收分片，中断后据此续传。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadSession {
    pub upload_id: String,
    pub file_size: i64,
    pub chunk_size: i64,
    pub received: Vec<u32>,
}

impl UploadSession {
    /// 由 init 响应建立会话；过滤越界/重复分片，保证 received 是 [0,total) 内的去重升序集合。
    pub fn from_init(size: i64, init: &InitResponse) -> Result<Self, TransferError> {
        if size < 0 || init.chunk_size < 1 {
            return Err(TransferError::InvalidParam);
        }
        let total = total_chunks(size, init.chunk_size)?;
        let mut received: Vec<u32> = init
            .received_chunks
            .iter()
            .copied()
            .filter(|i| *i < total)
            .collect();
        received.sort_unstable();
        received.dedup();
        Ok(Self {
            upload_id: init.upload_id.clone(),
            file_size: size,
            chunk_size: init.chunk_size,
            received,
        })
    }

    /// 中断重启后，用新的 init 响应刷新会话（upload_id 可沿用服务端返回的）。
    /// 分片大小必须与首次一致，否则契约不符 → 4003/ContractMismatch。
    pub fn resume(&mut self, init: &InitResponse) -> Result<(), TransferError> {
        if init.chunk_size != self.chunk_size {
            return Err(TransferError::ContractMismatch);
        }
        let refreshed = Self::from_init(self.file_size, init)?;
        self.upload_id = refreshed.upload_id;
        // 合并本地已确认与服务端已收，去重升序。
        let mut merged: Vec<u32> = self
            .received
            .iter()
            .copied()
            .chain(refreshed.received)
            .collect();
        merged.sort_unstable();
        merged.dedup();
        self.received = merged;
        Ok(())
    }

    /// 仍待上传的分片索引（0..total 去掉 received），升序。
    pub fn pending_chunks(&self) -> Result<Vec<u32>, TransferError> {
        let total = total_chunks(self.file_size, self.chunk_size)?;
        Ok((0..total).filter(|i| !self.received.contains(i)).collect())
    }

    /// 标记某分片上传完成（幂等）。
    pub fn mark_received(&mut self, index: u32) -> Result<(), TransferError> {
        let total = total_chunks(self.file_size, self.chunk_size)?;
        if index >= total {
            return Err(TransferError::InvalidParam);
        }
        if !self.received.contains(&index) {
            self.received.push(index);
            self.received.sort_unstable();
        }
        Ok(())
    }

    /// 所有分片齐备（可 complete）。
    pub fn is_complete(&self) -> Result<bool, TransferError> {
        let total = total_chunks(self.file_size, self.chunk_size)?;
        Ok(self.received.len() == total as usize)
    }
}

/// 计算文件总片数（向上取整）；size=0 → 0 片。
pub fn total_chunks(size: i64, chunk_size: i64) -> Result<u32, TransferError> {
    if size < 0 || chunk_size < 1 {
        return Err(TransferError::InvalidParam);
    }
    if size == 0 {
        return Ok(0);
    }
    Ok(((size + chunk_size - 1) / chunk_size) as u32)
}

/// 某片字节区间 [offset, offset+len)。
pub fn chunk_range(size: i64, chunk_size: i64, index: u32) -> Result<(i64, usize), TransferError> {
    let total = total_chunks(size, chunk_size)?;
    if index >= total {
        return Err(TransferError::InvalidParam);
    }
    let offset = index as i64 * chunk_size;
    let remain = size - offset;
    Ok((offset, remain.min(chunk_size) as usize))
}

/// 某片 sha256（X-Chunk-Sha256 头）：对齐服务端 put_chunk 校验。
pub fn chunk_sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

// ---- 下载：Range 断点续传 ----

/// 下载进度：已落盘字节数 → 下一段 Range（bytes=start-end，闭区间）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadResume {
    pub total_size: i64,
    pub downloaded: i64,
}

impl DownloadResume {
    pub fn new(total_size: i64, downloaded: i64) -> Result<Self, TransferError> {
        if total_size < 0 || downloaded < 0 || downloaded > total_size {
            return Err(TransferError::InvalidParam);
        }
        Ok(Self { total_size, downloaded })
    }

    /// 下一段 Range：返回 (start, end)；已完成返回 None。
    /// max_chunk 为单次下载上限（防止一次请求过大）。
    pub fn next_range(&self, max_chunk: i64) -> Result<Option<(i64, i64)>, TransferError> {
        if max_chunk < 1 {
            return Err(TransferError::InvalidParam);
        }
        if self.downloaded >= self.total_size {
            return Ok(None);
        }
        let end = (self.downloaded + max_chunk - 1).min(self.total_size - 1);
        Ok(Some((self.downloaded, end)))
    }

    pub fn is_complete(&self) -> bool {
        self.downloaded >= self.total_size
    }
}

/// 解析响应 Range 头里的 `bytes start-end/total` 里的已下字节数（用于续传）。
pub fn downloaded_from_content_range(header: &str) -> Result<i64, TransferError> {
    // 形如 "bytes 0-1023/4096" → end+1 = 1024。
    let spec = header
        .strip_prefix("bytes ")
        .ok_or(TransferError::InvalidParam)?;
    let range = spec.split('/').next().ok_or(TransferError::InvalidParam)?;
    let end = range
        .split('-')
        .nth(1)
        .ok_or(TransferError::InvalidParam)?
        .parse::<i64>()
        .map_err(|_| TransferError::InvalidParam)?;
    Ok(end + 1)
}

// ---- 回收站 ----

/// 软删除项是否过期（recycled_at 与 now 均为 epoch 秒）。
pub fn recycle_expired(recycled_at: i64, now: i64, retention_days: i64) -> bool {
    if retention_days < 0 {
        return false;
    }
    now.saturating_sub(recycled_at) >= retention_days.saturating_mul(24 * 3600)
}

/// 默认回收站过期判定（30 天）。
pub fn recycle_expired_default(recycled_at: i64, now: i64) -> bool {
    recycle_expired(recycled_at, now, RECYCLE_RETENTION_DAYS)
}
