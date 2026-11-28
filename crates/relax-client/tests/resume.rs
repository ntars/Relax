//! C4.5 验收：客户端断点续传（init 只传缺失片 / 中断重启续传 / Range 下载 / 回收站）。

use std::error::Error;

use relax_client::transfer::{
    chunk_range, chunk_sha256, downloaded_from_content_range, recycle_expired_default,
    total_chunks, DownloadResume, InitResponse, TransferError, UploadSession,
    DEFAULT_CHUNK_SIZE, RECYCLE_RETENTION_DAYS,
};

fn init(upload_id: &str, chunk_size: i64, received: &[u32]) -> InitResponse {
    InitResponse {
        upload_id: upload_id.to_string(),
        chunk_size,
        received_chunks: received.to_vec(),
        expires_at: "2026-01-08T00:00:00Z".to_string(),
    }
}

#[test]
fn total_chunks_rounds_up_and_zero_size() -> Result<(), Box<dyn Error>> {
    assert_eq!(total_chunks(0, 10)?, 0);
    assert_eq!(total_chunks(25, 10)?, 3);
    assert_eq!(total_chunks(20, 10)?, 2);
    assert!(matches!(total_chunks(-1, 10), Err(TransferError::InvalidParam)));
    assert!(matches!(total_chunks(10, 0), Err(TransferError::InvalidParam)));
    Ok(())
}

#[test]
fn init_only_missing_chunks_are_pending() -> Result<(), Box<dyn Error>> {
    // 5 片文件，服务端已收 [0,2]，只传 [1,3,4]。
    let session = UploadSession::from_init(5 * 10, &init("u1", 10, &[0, 2]))?;
    assert_eq!(session.pending_chunks()?, vec![1, 3, 4]);
    assert!(!session.is_complete()?);
    Ok(())
}

#[test]
fn init_filters_out_of_range_and_duplicate_chunks() -> Result<(), Box<dyn Error>> {
    // 收到 [0, 0, 2, 99(越界)] → 去重过滤后 [0, 2]。
    let session = UploadSession::from_init(30, &init("u1", 10, &[0, 0, 2, 99]))?;
    assert_eq!(session.received, vec![0, 2]);
    assert_eq!(session.pending_chunks()?, vec![1]);
    Ok(())
}

#[test]
fn interrupt_then_resume_narrows_pending_to_newly_received() -> Result<(), Box<dyn Error>> {
    // 首轮 init 收到 [0]，传了片 1。
    let mut session = UploadSession::from_init(40, &init("u1", 10, &[0]))?;
    session.mark_received(1)?;
    assert_eq!(session.pending_chunks()?, vec![2, 3]);

    // 中断重启：服务端 received_chunks_json 已持久化 [0,1,3]，续传只传片 2。
    session.resume(&init("u1", 10, &[0, 1, 3]))?;
    assert_eq!(session.received, vec![0, 1, 3]);
    assert_eq!(session.pending_chunks()?, vec![2], "续传不从 0 开始，只补缺失片");
    Ok(())
}

#[test]
fn resume_with_different_chunk_size_is_contract_mismatch() -> Result<(), Box<dyn Error>> {
    let mut session = UploadSession::from_init(40, &init("u1", 10, &[0]))?;
    let err = session.resume(&init("u1", 20, &[0])).unwrap_err();
    assert!(matches!(err, TransferError::ContractMismatch));
    Ok(())
}

#[test]
fn mark_received_is_idempotent_and_complete_when_all() -> Result<(), Box<dyn Error>> {
    let mut session = UploadSession::from_init(20, &init("u1", 10, &[]))?;
    session.mark_received(0)?;
    session.mark_received(0)?; // 重复幂等
    assert_eq!(session.received, vec![0]);
    session.mark_received(1)?;
    assert!(session.is_complete()?);
    assert!(session.pending_chunks()?.is_empty());
    assert!(matches!(
        session.mark_received(9),
        Err(TransferError::InvalidParam)
    ));
    Ok(())
}

#[test]
fn chunk_range_covers_last_partial_chunk() -> Result<(), Box<dyn Error>> {
    // 25 字节 / 10 分片 = [0,10), [10,20), [20,25)。
    assert_eq!(chunk_range(25, 10, 0)?, (0, 10));
    assert_eq!(chunk_range(25, 10, 2)?, (20, 5));
    assert!(matches!(chunk_range(25, 10, 3), Err(TransferError::InvalidParam)));
    Ok(())
}

#[test]
fn chunk_sha256_matches_service_side() {
    // 与 relax-server storage::sha256_hex 同一算法（X-Chunk-Sha256 头）。
    assert_eq!(
        chunk_sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn download_resume_continues_from_downloaded_bytes() -> Result<(), Box<dyn Error>> {
    let r = DownloadResume::new(4096, 1024)?;
    assert_eq!(r.next_range(512)?, Some((1024, 1535)));
    assert!(!r.is_complete());

    let done = DownloadResume::new(4096, 4096)?;
    assert_eq!(done.next_range(512)?, None);
    assert!(done.is_complete());
    assert!(matches!(
        DownloadResume::new(100, 200),
        Err(TransferError::InvalidParam)
    ));
    Ok(())
}

#[test]
fn content_range_parses_downloaded_bytes() -> Result<(), Box<dyn Error>> {
    assert_eq!(downloaded_from_content_range("bytes 0-1023/4096")?, 1024);
    assert_eq!(downloaded_from_content_range("bytes 2048-4095/4096")?, 4096);
    assert!(matches!(
        downloaded_from_content_range("bytes bad"),
        Err(TransferError::InvalidParam)
    ));
    Ok(())
}

#[test]
fn recycle_expires_after_default_30_days() {
    let day = 24 * 3600;
    assert!(!recycle_expired_default(1000, 1000 + 29 * day));
    assert!(recycle_expired_default(1000, 1000 + 30 * day));
    assert_eq!(RECYCLE_RETENTION_DAYS, 30);
    assert_eq!(DEFAULT_CHUNK_SIZE, 2 * 1024 * 1024);
}
