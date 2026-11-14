//! RCD-02 blob SHA-256 digest tests (synvelis#116 slice).
//!
//! `publish_recorded` の canonical blob が SHA-256 content address
//! （`"sha256:<64hex>"` → `blobs/sha256/<hex>`）で publish され、
//! receipt/commit/playback が同一 identity を運ぶこと、legacy
//! CRC32 receipt が flat blob で維持されること、publish 効果不明を
//! 観測 reconcile することを headless で検証する。
//! fake/injected fault の成功は実 FS durability の証明にしない。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_io_file::{DurabilityEvidence, PublishEffect, PublishFailureKind, PublishPhase};
use sha2::Digest as _;
use synvelis_library::{
    BlobPublishError, BlobPublishFailure, BlobPublisher, DirectorySync, PrincipalId,
    ProviderContext, PublishDurability,
};
use synvelis_player::import::{blob_path, content_digest};
use synvelis_player::publish::{PublishError, publish_recorded, publish_recorded_with};
use synvelis_player::{PublishReport, ResolveError, open_blob_verified};
use synvelis_project::RecordingSessionId;
use synvelis_recording::staging::{
    CANONICAL_MEDIA_FORMAT, CANONICAL_SCHEMA_VERSION, CanonicalSourceRef, PCM_FORMAT,
    STAGING_SCHEMA_VERSION, StagingArea, StagingSessionConfig, StagingStreamConfig, media_rel,
};
use synvelis_recording::writer::{FinalizedOutcome, FinalizedSourceRef};

const RATE: u32 = 48_000;
const CHANNELS: u32 = 1;
const FRAMES: usize = 100;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "synvelis-rcd02-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    if d.exists() {
        fs::remove_dir_all(&d).unwrap();
    }
    fs::create_dir_all(&d).unwrap();
    d
}

/// 100 frames mono f32le の決定的 media byte 列（sample 値は
/// publish が検証しないため bit pattern のみ決定的であればよい）。
fn media_bytes() -> Vec<u8> {
    (0..FRAMES)
        .flat_map(|i| {
            u32::try_from(i)
                .unwrap()
                .wrapping_mul(0x9E37_79B1)
                .to_le_bytes()
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = sha2::Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push(HEX[usize::from(b >> 4)] as char);
        out.push(HEX[usize::from(b & 0x0f)] as char);
    }
    out
}

/// manifest + media + Complete finalized ref を持つ staging session
/// を構築する（writer/recovery を介さない最小 publish 入力）。
fn setup_session(dir: &Path, stream_index: u16) -> (StagingArea, FinalizedSourceRef) {
    let session_id = RecordingSessionId::new(1);
    let area = StagingArea::create(
        dir,
        &StagingSessionConfig {
            session_id,
            sample_rate: RATE,
            streams: vec![StagingStreamConfig {
                stream_index,
                capture_stream_id: "capture-stream:test".to_string(),
                lane_id: "lane:test".to_string(),
                channels: CHANNELS,
            }],
        },
    )
    .unwrap();
    let media = media_bytes();
    fs::write(area.media_path(stream_index), &media).unwrap();
    let fin = FinalizedSourceRef {
        schema_version: STAGING_SCHEMA_VERSION,
        session_id: session_id.to_tagged(),
        stream_index,
        capture_stream_id: "capture-stream:test".to_string(),
        lane_id: "lane:test".to_string(),
        sample_rate: RATE,
        pcm_format: PCM_FORMAT.to_string(),
        channels: CHANNELS,
        media: media_rel(stream_index),
        media_byte_len: media.len() as u64,
        source_frames: FRAMES as u64,
        outcome: FinalizedOutcome::Complete,
        spans: vec![],
        digest: content_digest(&media),
        verified_sha256: Some(format!("sha256:{}", sha256_hex(&media))),
        journal_seq: 1,
        checkpoint_index: 0,
        faults: vec![],
        durability_evidence: vec![],
    };
    fs::write(
        area.finalized_path(stream_index),
        serde_json::to_vec(&fin).unwrap(),
    )
    .unwrap();
    (area, fin)
}

fn receipt(area: &StagingArea, stream_index: u16) -> CanonicalSourceRef {
    let bytes = fs::read(area.canonical_path(stream_index)).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn write_receipt(area: &StagingArea, stream_index: u16, can: &CanonicalSourceRef) {
    fs::write(
        area.canonical_path(stream_index),
        serde_json::to_vec_pretty(can).unwrap(),
    )
    .unwrap();
}

fn legacy_receipt(fin: &FinalizedSourceRef, digest: &str, byte_len: u64) -> CanonicalSourceRef {
    CanonicalSourceRef {
        schema_version: CANONICAL_SCHEMA_VERSION,
        session_id: fin.session_id.clone(),
        stream_index: fin.stream_index,
        capture_stream_id: fin.capture_stream_id.clone(),
        lane_id: fin.lane_id.clone(),
        raw_digest: fin.digest.clone(),
        digest: digest.to_string(),
        media_format: CANONICAL_MEDIA_FORMAT.to_string(),
        byte_len,
        source_frames: fin.source_frames,
        sample_rate: fin.sample_rate,
        channels: fin.channels,
    }
}

fn test_cx() -> ProviderContext {
    ProviderContext::new(PrincipalId::new("rcd02-test").unwrap())
}

// ---------- E1: 新規 publish は sha256 content address ----------

#[test]
fn publish_writes_sha256_blob_and_receipt() {
    let dir = scratch("publish-sha256");
    let media_root = dir.join("media");
    let (area, fin) = setup_session(&dir.join("staging"), 0);

    let report = publish_recorded(&area, &media_root).expect("publish_recorded");
    assert_eq!(report.published.len(), 1);
    assert!(matches!(report.reused.as_slice(), []));
    assert!(matches!(report.skipped.as_slice(), []));

    let p = &report.published[0];
    assert!(
        p.digest.starts_with("sha256:"),
        "canonical digest must be sha256 content address: {}",
        p.digest
    );
    assert_eq!(p.digest.len(), "sha256:".len() + 64);
    assert!(
        p.digest["sha256:".len()..]
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    );

    // blob は `blobs/sha256/<hex>` 名前空間に置かれる。
    let hex = &p.digest["sha256:".len()..];
    let expected_blob = media_root.join("blobs").join("sha256").join(hex);
    assert_eq!(p.blob, expected_blob);
    assert!(expected_blob.exists());
    // flat legacy 名前空間には置かれない。
    assert!(!media_root.join("blobs").join(hex).exists());

    // blob bytes は canonical PCM f32 WAV。
    let bytes = fs::read(&expected_blob).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");

    // receipt は同一 digest を運び、raw provenance を保持する。
    let can = receipt(&area, 0);
    assert_eq!(can.digest, p.digest);
    assert_eq!(can.raw_digest, fin.digest);
    assert_eq!(can.byte_len, p.byte_len);

    // open_blob_verified が同じ verified boundary で round-trip する。
    let verified = open_blob_verified(&media_root, &p.digest).expect("open_blob_verified");
    assert_eq!(verified.path, expected_blob);

    // publish 用 staged source は除去済み。
    assert!(!area.session_dir.join(".publish-0000.wav").exists());
}

// ---------- E4: 冪等 republish ----------

#[test]
fn republish_is_idempotent_and_reuses_verified_blob() {
    let dir = scratch("republish");
    let media_root = dir.join("media");
    let (area, _) = setup_session(&dir.join("staging"), 0);

    let first = publish_recorded(&area, &media_root).expect("first publish");
    assert_eq!(first.published.len(), 1);
    let digest = first.published[0].digest.clone();
    let blob = first.published[0].blob.clone();
    let blob_bytes = fs::read(&blob).unwrap();

    let second = publish_recorded(&area, &media_root).expect("second publish");
    assert!(matches!(second.published.as_slice(), []));
    assert_eq!(second.reused.len(), 1);
    assert_eq!(second.reused[0].digest, digest);
    assert_eq!(second.reused[0].blob, blob);
    assert_eq!(fs::read(&blob).unwrap(), blob_bytes, "blob bytes unchanged");
}

// ---------- E5: 欠損 blob の再公開 ----------

#[test]
fn missing_sha256_blob_is_republished() {
    let dir = scratch("missing-blob");
    let media_root = dir.join("media");
    let (area, _) = setup_session(&dir.join("staging"), 0);

    let first = publish_recorded(&area, &media_root).expect("first publish");
    let digest = first.published[0].digest.clone();
    let blob = first.published[0].blob.clone();
    fs::remove_file(&blob).unwrap();

    let second = publish_recorded(&area, &media_root).expect("republish");
    assert!(
        matches!(second.published.as_slice(), []),
        "receipt 整合 → reused"
    );
    assert_eq!(second.reused.len(), 1);
    assert_eq!(second.reused[0].digest, digest);
    assert!(blob.exists(), "blob re-materialized");
    open_blob_verified(&media_root, &digest).expect("re-published blob verifies");
}

// ---------- E2: tamper は fail-closed ----------

#[test]
fn tampered_sha256_blob_fails_closed() {
    let dir = scratch("tamper");
    let media_root = dir.join("media");
    let (area, _) = setup_session(&dir.join("staging"), 0);

    let report = publish_recorded(&area, &media_root).expect("publish");
    let p = &report.published[0];

    // blob bytes を改竄 → verified boundary が HashMismatch。
    let mut bytes = fs::read(&p.blob).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xFF;
    fs::write(&p.blob, &bytes).unwrap();
    assert!(matches!(
        open_blob_verified(&media_root, &p.digest),
        Err(ResolveError::HashMismatch { .. })
    ));

    // publish 側も byte 再検証で fail-closed（別内容で上書きしない）。
    let err = publish_recorded(&area, &media_root).expect_err("tampered blob must fail");
    assert!(
        matches!(err, PublishError::CorruptMedia { .. }),
        "expected CorruptMedia, got {err:?}"
    );
    // 改竄 bytes はそのまま残る（silent 修復・置換をしない）。
    assert_eq!(fs::read(&p.blob).unwrap(), bytes);
}

// ---------- E3: legacy CRC32 receipt は flat blob で維持 ----------

#[test]
fn legacy_crc32_receipt_keeps_flat_blob() {
    let dir = scratch("legacy-crc32");
    let media_root = dir.join("media");
    let (area, fin) = setup_session(&dir.join("staging"), 0);

    // 昇格前形式の receipt + flat blob を直接配置する。
    // blob 内容は canonical WAV である必要はない — reuse 判定は
    // CRC32 fold の byte 再検証のみ。
    let blob_bytes = b"legacy-canonical-wav-bytes".to_vec();
    let digest = content_digest(&blob_bytes);
    assert_eq!(digest.len(), 16, "legacy digest は hex16");
    let flat = blob_path(&media_root, &digest);
    fs::create_dir_all(flat.parent().unwrap()).unwrap();
    fs::write(&flat, &blob_bytes).unwrap();
    write_receipt(
        &area,
        0,
        &legacy_receipt(&fin, &digest, blob_bytes.len() as u64),
    );

    let report = publish_recorded(&area, &media_root).expect("publish_recorded");
    assert_eq!(report.reused.len(), 1);
    assert!(matches!(report.published.as_slice(), []));
    let p = &report.reused[0];
    assert_eq!(p.digest, digest, "legacy digest を sha256 へ書き換えない");
    assert_eq!(p.blob, flat, "flat blob path を維持する");

    // sha256 名前空間は作られない。
    assert!(!media_root.join("blobs").join("sha256").exists());
    // open_blob_verified は legacy digest を flat path + CRC32 で解決する。
    open_blob_verified(&media_root, &digest).expect("legacy blob verifies");
}

// ---------- E7: opaque digest は fail-closed ----------

#[test]
fn opaque_receipt_digest_fails_closed() {
    let dir = scratch("opaque");
    let media_root = dir.join("media");
    let (area, fin) = setup_session(&dir.join("staging"), 0);

    write_receipt(&area, 0, &legacy_receipt(&fin, "not-a-content-digest", 0));
    let err = publish_recorded(&area, &media_root).expect_err("opaque digest must fail");
    assert!(
        matches!(err, PublishError::CorruptMedia { .. }),
        "expected CorruptMedia, got {err:?}"
    );
}

// ---------- E5b: indeterminate publish を観測 reconcile ----------

/// publish 成功相当の bytes を destination へ書いた上で
/// `PublishEffect::Indeterminate` を報告する stub。
struct LandThenReport {
    effect: PublishEffect,
    calls: AtomicU64,
}

impl BlobPublisher for LandThenReport {
    fn publish(
        &self,
        staged_source: &Path,
        _expected: &synvelis_library::ContentDigest,
        _expected_len: u64,
        destination: &Path,
        _durability: PublishDurability,
        _cx: &ProviderContext,
    ) -> Result<synvelis_library::BlobPublishReceipt, BlobPublishError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if let Some(dir) = destination.parent() {
            fs::create_dir_all(dir).unwrap();
        }
        fs::copy(staged_source, destination).unwrap();
        Err(BlobPublishError::Publish(BlobPublishFailure {
            kind: PublishFailureKind::Io,
            phase: PublishPhase::DirectorySync,
            effect: self.effect,
            durability: DurabilityEvidence::FileSynced,
            directory_sync: DirectorySync::Unknown,
            detail: "injected post-publish failure".to_string(),
        }))
    }
}

#[test]
fn indeterminate_publish_is_reconciled_by_observation() {
    let dir = scratch("indeterminate");
    let media_root = dir.join("media");
    let (area, _) = setup_session(&dir.join("staging"), 0);

    let publisher = LandThenReport {
        effect: PublishEffect::Indeterminate,
        calls: AtomicU64::new(0),
    };
    let report = publish_recorded_with(&area, &media_root, &publisher, &test_cx())
        .expect("indeterminate-but-landed publish reconciles");
    assert_eq!(report.published.len(), 1);
    // 着地済み bytes は verified boundary で再検証できる。
    let p = &report.published[0];
    open_blob_verified(&media_root, &p.digest).expect("landed blob verifies");
    // destination が観測で確認できたため retry は走らない。
    assert_eq!(publisher.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn published_effect_failure_is_reconciled_by_observation() {
    let dir = scratch("published-effect");
    let media_root = dir.join("media");
    let (area, _) = setup_session(&dir.join("staging"), 0);

    let publisher = LandThenReport {
        effect: PublishEffect::Published,
        calls: AtomicU64::new(0),
    };
    let report = publish_recorded_with(&area, &media_root, &publisher, &test_cx())
        .expect("published-effect failure reconciles");
    assert_eq!(report.published.len(), 1);
    assert_eq!(publisher.calls.load(Ordering::Relaxed), 1);
}

/// destination に何も書かず失敗を報告する stub — 未公開確定 →
/// 1 回だけ retry して最終的に失敗を返す。
struct AlwaysFail {
    calls: AtomicU64,
}

impl BlobPublisher for AlwaysFail {
    fn publish(
        &self,
        _staged_source: &Path,
        _expected: &synvelis_library::ContentDigest,
        _expected_len: u64,
        _destination: &Path,
        _durability: PublishDurability,
        _cx: &ProviderContext,
    ) -> Result<synvelis_library::BlobPublishReceipt, BlobPublishError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(BlobPublishError::Publish(BlobPublishFailure {
            kind: PublishFailureKind::Io,
            phase: PublishPhase::Publish,
            effect: PublishEffect::NotPublished,
            durability: DurabilityEvidence::None,
            directory_sync: DirectorySync::NotRequested,
            detail: "injected publish denial".to_string(),
        }))
    }
}

#[test]
fn unlanded_publish_retries_once_then_fails_closed() {
    let dir = scratch("always-fail");
    let media_root = dir.join("media");
    let (area, _) = setup_session(&dir.join("staging"), 0);

    let publisher = AlwaysFail {
        calls: AtomicU64::new(0),
    };
    let err = publish_recorded_with(&area, &media_root, &publisher, &test_cx())
        .expect_err("unlanded publish must fail");
    assert!(
        matches!(err, PublishError::Io { .. }),
        "expected Io, got {err:?}"
    );
    // 未公開確定 → 1 回だけ retry（無限 retry しない）。
    assert_eq!(publisher.calls.load(Ordering::Relaxed), 2);
    // blob も receipt も残らない（fail-closed・部分 publish なし）。
    assert!(!media_root.join("blobs").join("sha256").exists());
    assert!(!area.canonical_path(0).exists());
    // staged source は除去済み。
    assert!(!area.session_dir.join(".publish-0000.wav").exists());
}

// ---------- report shape ----------

#[test]
fn report_carries_provenance_fields() {
    let dir = scratch("provenance");
    let media_root = dir.join("media");
    let (area, fin) = setup_session(&dir.join("staging"), 0);

    let report: PublishReport = publish_recorded(&area, &media_root).expect("publish");
    let p = &report.published[0];
    // raw media fold digest は canonical digest と別 namespace で保持。
    assert_eq!(p.raw_digest, fin.digest);
    assert_eq!(p.raw_digest.len(), 16);
    assert_ne!(p.raw_digest, p.digest);
    // finalized ref の verified_sha256 provenance は receipt 経路で
    // commit manifest の raw provenance へ伝わる（commit 側の
    // StreamEntry が digest_scheme=ContentDigest + digest=sha256 を
    // 採用することは synvelis-recording の commit test が担保）。
    let can = receipt(&area, 0);
    assert_eq!(can.source_frames, fin.source_frames);
    assert_eq!(can.sample_rate, fin.sample_rate);
    assert_eq!(can.channels, fin.channels);
}
