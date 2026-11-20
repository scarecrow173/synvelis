//! Issue #601 slice 1 (D-layer formal verification) — staging journal
//! wire codec + crc32/digest fold + session state-machine property
//! suite, plus `#[cfg(kani)]` bounded proof harnesses.
//!
//! Target contracts (product source of truth:
//! `crates/synvelis-recording/src/journal.rs` for the frame codec,
//! `src/crc32.rs` for crc/digest, `src/session.rs` for the lifecycle
//! state machine):
//!
//! - `replay_journal(&[u8]) -> ReplayOutcome` never panics; `stop`
//!   stays in the closed `StopReason` set (compile-time exhaustive);
//!   `prefix_len` never exceeds the input length; `CleanEnd` iff every
//!   byte verified; verified record seqs are contiguous from 0 and
//!   `last_seq` is the last verified record's seq; the verified prefix
//!   itself always replays to `CleanEnd`.
//! - `encode_record` emits the documented layout (magic/version/kind/
//!   seq/len/payload/crc32), is `Some` for `payload_len <= MAX_PAYLOAD`
//!   and `None` above it; `JournalAppender::encode_next` owns seq
//!   discipline and a contiguous stream replays to `CleanEnd` with
//!   identical `RecordPayload`s.
//! - Every byte of a frame is covered by either a direct field check
//!   or the trailing CRC, so a single-byte corruption inside record k
//!   can never replay as intact past record k.
//! - `crc32` (one-shot) and `Crc32::{new,update,finish}` (incremental)
//!   agree over arbitrary splits; `digest_fold`/`IntegrityDigest` folds
//!   are deterministic and render as fixed-width lowercase hex.
//! - `legal_transition`/`require_transition` agree on the 4×4 pair
//!   space; `accepts_registration` holds exactly in `Open` and
//!   `accepts_take_verification` exactly in `Open | Closed`.
//!
//! The `#[cfg(kani)]` module at the bottom carries the same contracts
//! as bounded proofs (run via `mise run test-formal-kani`); the
//! bounds and assumptions are documented in
//! `docs/specs/formal-verification.md`.

use proptest::prelude::*;
use synvelis_project::SessionState;
use synvelis_recording::journal::{
    CheckpointRecord, ChunkRecord, FinalizeOutcome, FinalizeRecord, HEADER_LEN, JournalAppender,
    JournalSpan, MAGIC, RecordPayload, StopReason, VERSION, encode_record, replay_journal,
};
use synvelis_recording::{
    Crc32, IntegrityDigest, accepts_registration, accepts_take_verification, crc32, digest_fold,
    legal_transition, require_transition,
};

// ---- record payload strategies ----

/// Arbitrary `JournalSpan`. Bounded structurally by the encoder
/// contract — span counts stay far below the `u16` count-field cap,
/// whose overflow path gets its own dedicated `#[test]`.
fn arb_journal_span() -> impl Strategy<Value = JournalSpan> {
    (
        any::<u32>(),
        any::<i64>(),
        any::<u32>(),
        any::<i64>(),
        any::<i64>(),
        any::<i64>(),
        any::<u64>(),
    )
        .prop_map(
            |(
                pass_index,
                session_start,
                frame_count,
                source_start,
                project_in,
                project_out,
                media_offset,
            )| {
                JournalSpan {
                    pass_index,
                    session_start,
                    frame_count,
                    source_start,
                    project_in,
                    project_out,
                    media_offset,
                }
            },
        )
}

fn arb_chunk_record() -> impl Strategy<Value = ChunkRecord> {
    (
        (
            any::<u32>(),
            any::<u64>(),
            any::<i64>(),
            any::<u32>(),
            any::<u32>(),
        ),
        (any::<u64>(), any::<u64>(), any::<u32>(), any::<u64>()),
        prop::collection::vec(arb_journal_span(), 0..=6),
    )
        .prop_map(
            |(
                (chunk_id, first_input_frame, session_start, block_frames, stored_frames),
                (media_offset, media_len, pcm_crc32, digest_fold),
                spans,
            )| {
                ChunkRecord {
                    chunk_id,
                    first_input_frame,
                    session_start,
                    block_frames,
                    stored_frames,
                    media_offset,
                    media_len,
                    pcm_crc32,
                    digest_fold,
                    spans,
                }
            },
        )
}

fn arb_checkpoint_record() -> impl Strategy<Value = CheckpointRecord> {
    (
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
        any::<u32>(),
    )
        .prop_map(
            |(checkpoint_index, media_len, source_frames, digest_fold, chunk_records)| {
                CheckpointRecord {
                    checkpoint_index,
                    media_len,
                    source_frames,
                    digest_fold,
                    chunk_records,
                }
            },
        )
}

fn arb_finalize_record() -> impl Strategy<Value = FinalizeRecord> {
    (any::<u64>(), any::<u64>(), any::<u64>(), any::<bool>()).prop_map(
        |(media_len, source_frames, digest_fold, partial)| FinalizeRecord {
            media_len,
            source_frames,
            digest_fold,
            outcome: if partial {
                FinalizeOutcome::Partial
            } else {
                FinalizeOutcome::Complete
            },
        },
    )
}

/// All three record kinds, with bounded span vectors.
fn arb_record_payload() -> impl Strategy<Value = RecordPayload> {
    prop_oneof![
        arb_chunk_record().prop_map(RecordPayload::Chunk),
        arb_checkpoint_record().prop_map(RecordPayload::Checkpoint),
        arb_finalize_record().prop_map(RecordPayload::Finalize),
    ]
}

/// A stream of encoded records with contiguous seq from 0, plus the
/// byte offsets where each record starts.
fn encode_stream(payloads: &[RecordPayload]) -> (Vec<u8>, Vec<usize>) {
    let mut stream = Vec::new();
    let mut starts = Vec::new();
    for (i, p) in payloads.iter().enumerate() {
        starts.push(stream.len());
        let seq = u64::try_from(i).unwrap_or(u64::MAX);
        stream.extend_from_slice(&encode_record(seq, p).expect("in-cap payload must encode"));
    }
    (stream, starts)
}

fn arb_state() -> impl Strategy<Value = SessionState> {
    prop::sample::select(vec![
        SessionState::Created,
        SessionState::Open,
        SessionState::Closed,
        SessionState::Finalized,
    ])
}

proptest! {
    /// For any byte input `replay_journal` terminates without panic,
    /// `stop` stays in the closed `StopReason` set, `prefix_len` never
    /// exceeds the input length, `CleanEnd` holds iff every byte
    /// verified, verified seqs are contiguous from 0, and `last_seq`
    /// mirrors the last verified record. The verified prefix alone
    /// always replays to `CleanEnd` with the same records.
    #[test]
    fn replay_journal_arbitrary_bytes_contract(
        bytes in prop::collection::vec(any::<u8>(), 0..=512),
    ) {
        let out = replay_journal(&bytes);
        prop_assert!(out.prefix_len <= bytes.len());
        prop_assert_eq!(out.stop == StopReason::CleanEnd, out.prefix_len == bytes.len());
        for (i, rec) in out.records.iter().enumerate() {
            prop_assert_eq!(rec.seq, u64::try_from(i).unwrap_or(u64::MAX));
        }
        prop_assert_eq!(out.last_seq, out.records.last().map(|r| r.seq));
        let again = replay_journal(&bytes[..out.prefix_len]);
        prop_assert_eq!(again.stop, StopReason::CleanEnd);
        prop_assert_eq!(again.records, out.records);
    }

    /// `encode_record` emits the documented wire layout: LE magic,
    /// the current version, a kind byte in {1,2,3}, LE seq and
    /// payload_len, the payload verbatim, and a CRC32 over
    /// `frame[2..]` excluding the crc field itself.
    #[test]
    fn encode_record_layout(seq in any::<u64>(), payload in arb_record_payload()) {
        let frame = encode_record(seq, &payload).expect("in-cap payload must encode");
        prop_assert_eq!(&frame[0..2], &MAGIC.to_le_bytes());
        prop_assert_eq!(frame[2], VERSION);
        prop_assert!((1..=3).contains(&frame[3]), "kind byte {} out of set", frame[3]);
        prop_assert_eq!(&frame[4..12], &seq.to_le_bytes());
        let plen = u32::from_le_bytes(frame[12..16].try_into().unwrap_or_default());
        prop_assert_eq!(frame.len(), HEADER_LEN + usize::try_from(plen).unwrap_or(usize::MAX) + 4);
        let stored = u32::from_le_bytes(frame[frame.len() - 4..].try_into().unwrap_or_default());
        prop_assert_eq!(stored, crc32(&frame[2..frame.len() - 4]));
    }

    /// A record carrying seq 0 replays to `CleanEnd` returning the
    /// identical `RecordPayload` — encode/decode is the identity for
    /// all three kinds.
    #[test]
    fn encode_record_seq0_replays(payload in arb_record_payload()) {
        let frame = encode_record(0, &payload).expect("in-cap payload must encode");
        let out = replay_journal(&frame);
        prop_assert_eq!(out.stop, StopReason::CleanEnd);
        prop_assert_eq!(out.records.len(), 1);
        prop_assert_eq!(&out.records[0].payload, &payload);
        prop_assert_eq!(out.last_seq, Some(0));
    }

    /// A single record with a non-zero seq is a `SequenceGap` —
    /// replay only accepts streams that start at seq 0.
    #[test]
    fn encode_record_nonzero_seq_is_gap(
        seq in 1u64..=u64::MAX,
        payload in arb_record_payload(),
    ) {
        let frame = encode_record(seq, &payload).expect("in-cap payload must encode");
        let out = replay_journal(&frame);
        prop_assert_eq!(out.stop, StopReason::SequenceGap);
        prop_assert!(out.records.is_empty());
        prop_assert_eq!(out.prefix_len, 0);
        prop_assert_eq!(out.last_seq, None);
    }

    /// `JournalAppender` owns seq discipline: `next_seq` advances by
    /// one per encoded record, each emitted frame embeds the
    /// running seq, and the concatenated stream replays to `CleanEnd`
    /// with identical payloads in order.
    #[test]
    fn appender_stream_roundtrip_and_seq_discipline(
        payloads in prop::collection::vec(arb_record_payload(), 0..=8),
    ) {
        let mut ap = JournalAppender::new();
        let mut stream = Vec::new();
        for (i, p) in payloads.iter().enumerate() {
            prop_assert_eq!(ap.next_seq(), u64::try_from(i).unwrap_or(u64::MAX));
            let frame = ap.encode_next(p).expect("in-cap payload must encode");
            let embedded = u64::from_le_bytes(frame[4..12].try_into().unwrap_or_default());
            prop_assert_eq!(embedded, u64::try_from(i).unwrap_or(u64::MAX));
            stream.extend_from_slice(&frame);
        }
        prop_assert_eq!(ap.next_seq(), u64::try_from(payloads.len()).unwrap_or(u64::MAX));
        let out = replay_journal(&stream);
        prop_assert_eq!(out.stop, StopReason::CleanEnd);
        prop_assert_eq!(out.prefix_len, stream.len());
        prop_assert_eq!(out.records.len(), payloads.len());
        for (i, rec) in out.records.iter().enumerate() {
            prop_assert_eq!(&rec.payload, &payloads[i]);
        }
        prop_assert_eq!(
            out.last_seq,
            payloads.len().checked_sub(1).map(|i| u64::try_from(i).unwrap_or(u64::MAX)),
        );
    }

    /// `JournalAppender::resumed(n)` keeps appending from n — the
    /// emitted frames embed seq n, n+1, … — but such a stream alone is
    /// not replayable from seq 0: replay stops at the first record
    /// with `SequenceGap`. This pins the current "reopen mid-stream is
    /// not standalone-replayable" contract.
    #[test]
    fn resumed_appender_frames_gap_as_fresh_stream(
        start in 1u64..=u64::MAX,
        payloads in prop::collection::vec(arb_record_payload(), 1..=3),
    ) {
        let mut ap = JournalAppender::resumed(start);
        let mut stream = Vec::new();
        for p in &payloads {
            stream.extend_from_slice(&ap.encode_next(p).expect("in-cap payload must encode"));
        }
        prop_assert_eq!(
            ap.next_seq(),
            start.saturating_add(u64::try_from(payloads.len()).unwrap_or(u64::MAX)),
        );
        let out = replay_journal(&stream);
        prop_assert_eq!(out.stop, StopReason::SequenceGap);
        prop_assert!(out.records.is_empty());
        prop_assert_eq!(out.prefix_len, 0);
    }

    /// Every byte of a record is covered by a direct field check or
    /// the trailing CRC, so a single-byte corruption inside record k
    /// can never be verified as intact: replay must stop exactly at
    /// record k with only the earlier records verified.
    #[test]
    fn single_byte_corruption_never_verifies_past_it(
        payloads in prop::collection::vec(arb_record_payload(), 1..=4),
        rec_pick in any::<prop::sample::Index>(),
        ofs_pick in any::<prop::sample::Index>(),
        xor in 1u8..=255,
    ) {
        let (mut stream, starts) = encode_stream(&payloads);
        let k = rec_pick.index(payloads.len());
        let rec_len = encode_record(0, &payloads[k]).expect("in-cap").len();
        let ofs = ofs_pick.index(rec_len);
        stream[starts[k] + ofs] ^= xor;
        let out = replay_journal(&stream);
        prop_assert_eq!(out.records.len(), k, "corrupted record must not verify");
        prop_assert_eq!(
            out.prefix_len,
            starts[k],
            "prefix must end exactly at the corrupted record",
        );
        prop_assert_ne!(out.stop, StopReason::CleanEnd);
    }

    /// Truncating an encoded stream mid-record yields `TornTail`;
    /// truncating at a record boundary keeps `CleanEnd`. Verified
    /// records are exactly the frames fully inside the kept prefix.
    #[test]
    fn truncation_is_torn_tail_or_boundary_clean(
        payloads in prop::collection::vec(arb_record_payload(), 1..=4),
        cut_pick in any::<prop::sample::Index>(),
    ) {
        let (stream, starts) = encode_stream(&payloads);
        let ends: Vec<usize> = starts
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i + 1 < starts.len() {
                    starts[i + 1]
                } else {
                    stream.len()
                }
            })
            .collect();
        let cut = cut_pick.index(stream.len() + 1);
        let out = replay_journal(&stream[..cut]);
        let complete = ends.iter().filter(|&&e| e <= cut).count();
        let expected_prefix = complete.checked_sub(1).map_or(0, |i| ends[i]);
        let on_boundary = cut == stream.len() || starts.contains(&cut);
        prop_assert_eq!(out.records.len(), complete);
        prop_assert_eq!(out.prefix_len, expected_prefix);
        if on_boundary {
            prop_assert_eq!(out.stop, StopReason::CleanEnd);
        } else {
            prop_assert_eq!(out.stop, StopReason::TornTail);
        }
    }

    /// A valid stream followed by bytes that cannot start a frame
    /// stops exactly at the record boundary — trailing garbage is never
    /// absorbed into the verified prefix. Fewer than `HEADER_LEN`
    /// trailing bytes is `TornTail` (the header itself can't be read);
    /// ≥ `HEADER_LEN` with a non-magic first byte is `BadMagic`.
    /// `MAGIC.to_le_bytes()` puts `0x52` on the wire first.
    #[test]
    fn trailing_garbage_stops_at_boundary(
        payloads in prop::collection::vec(arb_record_payload(), 0..=4),
        first in any::<u8>(),
        tail in prop::collection::vec(any::<u8>(), 0..=48),
    ) {
        let (mut stream, _) = encode_stream(&payloads);
        let frames_len = stream.len();
        let first = if first == 0x52 { 0x53 } else { first };
        stream.push(first);
        stream.extend_from_slice(&tail);
        let out = replay_journal(&stream);
        let tail_len = 1 + tail.len();
        let expected = if tail_len < HEADER_LEN {
            StopReason::TornTail
        } else {
            StopReason::BadMagic
        };
        prop_assert_eq!(out.stop, expected);
        prop_assert_eq!(out.records.len(), payloads.len());
        prop_assert_eq!(out.prefix_len, frames_len);
    }

    /// The incremental `Crc32` folding agrees with the one-shot
    /// `crc32` over arbitrary split points.
    #[test]
    fn crc32_incremental_matches_oneshot(
        bytes in prop::collection::vec(any::<u8>(), 0..=256),
        splits in prop::collection::vec(any::<prop::sample::Index>(), 0..=4),
    ) {
        let mut points: Vec<usize> = splits.iter().map(|s| s.index(bytes.len() + 1)).collect();
        points.sort_unstable();
        points.dedup();
        let mut c = Crc32::new();
        let mut pos = 0;
        for &p in &points {
            c.update(&bytes[pos..p]);
            pos = p;
        }
        c.update(&bytes[pos..]);
        prop_assert_eq!(c.finish(), crc32(&bytes));
    }

    /// `digest_fold` chains are deterministic: folding the same value
    /// sequence from zero by raw `digest_fold` and by
    /// `IntegrityDigest::fold` agrees, and `to_hex` renders exactly 16
    /// lowercase hex chars.
    #[test]
    fn digest_fold_chain_is_deterministic(
        values in prop::collection::vec(any::<u64>(), 0..=16),
    ) {
        let raw = values
            .iter()
            .fold(0u64, |acc, &v| digest_fold(acc, v));
        let typed = values
            .iter()
            .fold(IntegrityDigest::ZERO, |acc, &v| acc.fold(v));
        prop_assert_eq!(raw, u64::from(typed));
        // fold again — identical input must fold identically
        let raw2 = values
            .iter()
            .fold(0u64, |acc, &v| digest_fold(acc, v));
        prop_assert_eq!(raw, raw2);
        let hex = typed.to_hex();
        prop_assert_eq!(hex.len(), 16);
        prop_assert!(hex.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    }

    /// `legal_transition` and `require_transition` agree over the
    /// full 4×4 pair space, and legality holds exactly on the three
    /// forward edges `Created→Open`, `Open→Closed`, `Closed→Finalized`.
    #[test]
    fn session_transition_agrees_with_require(from in arb_state(), to in arb_state()) {
        let legal = legal_transition(from, to);
        prop_assert_eq!(legal, require_transition(from, to).is_ok());
        let forward_edge = matches!(
            (from, to),
            (SessionState::Created, SessionState::Open)
                | (SessionState::Open, SessionState::Closed)
                | (SessionState::Closed, SessionState::Finalized)
        );
        prop_assert_eq!(legal, forward_edge);
    }

    /// The acceptance gates are total functions of the state:
    /// registration only in `Open`, take verification only in
    /// `Open | Closed`.
    #[test]
    fn session_acceptance_gates_are_total(state in arb_state()) {
        prop_assert_eq!(accepts_registration(state), state == SessionState::Open);
        prop_assert_eq!(
            accepts_take_verification(state),
            matches!(state, SessionState::Open | SessionState::Closed),
        );
    }
}

/// A Chunk whose span list pushes the record past `MAX_PAYLOAD` is
/// refused by `encode_record` — the `u16` count field saturates first,
/// but the byte size alone already exceeds the frame cap.
#[test]
fn chunk_record_over_cap_refused() {
    // 58-byte fixed payload + 48 bytes per span; 30000 spans ⇒
    // ~1.44 MiB > 1 MiB cap.
    let spans = vec![
        JournalSpan {
            pass_index: 0,
            session_start: 0,
            frame_count: 0,
            source_start: 0,
            project_in: 0,
            project_out: 0,
            media_offset: 0,
        };
        30_000
    ];
    let payload = RecordPayload::Chunk(ChunkRecord {
        chunk_id: 0,
        first_input_frame: 0,
        session_start: 0,
        block_frames: 0,
        stored_frames: 0,
        media_offset: 0,
        media_len: 0,
        pcm_crc32: 0,
        digest_fold: 0,
        spans,
    });
    assert!(encode_record(0, &payload).is_none());
}

/// The exact span-count boundary: the largest span list that still
/// fits under `MAX_PAYLOAD` encodes and round-trips; one span more is
/// refused.
#[test]
fn chunk_record_at_cap_boundary() {
    // fixed fields = 58 bytes; span = 48 bytes.
    // floor((1 MiB - 58) / 48) = 21844 → 58 + 48*21844 = 1_048_570 ≤ cap
    // 21845 → 1_048_618 > cap
    let make = |n: usize| {
        RecordPayload::Chunk(ChunkRecord {
            chunk_id: 0,
            first_input_frame: 0,
            session_start: 0,
            block_frames: 0,
            stored_frames: 0,
            media_offset: 0,
            media_len: 0,
            pcm_crc32: 0,
            digest_fold: 0,
            spans: vec![
                JournalSpan {
                    pass_index: 0,
                    session_start: 0,
                    frame_count: 0,
                    source_start: 0,
                    project_in: 0,
                    project_out: 0,
                    media_offset: 0,
                };
                n
            ],
        })
    };
    let payload = make(21_844);
    let frame = encode_record(0, &payload).expect("at-cap chunk must encode");
    let out = replay_journal(&frame);
    assert_eq!(out.stop, StopReason::CleanEnd);
    assert_eq!(out.records.len(), 1);
    assert_eq!(out.records[0].payload, payload);
    assert!(encode_record(0, &make(21_845)).is_none());
}

/// Bounded proofs of the same contracts, verified exhaustively over
/// symbolic input by Kani (`cargo kani --tests`; run via
/// `mise run test-formal-kani`). Bounds and assumptions:
/// `docs/specs/formal-verification.md`.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// Bounded stream length — covers two minimal records (each
    /// 16-byte header + 4-byte crc) plus slack so every `StopReason`
    /// path is reachable. Kept under 48 bytes so the Chunk decode path
    /// never reaches `Vec::with_capacity(span_count)` on a symbolic
    /// count (the `span_count` field sits at payload offset 26 —
    /// `payload < 28` fails earlier in the cursor reads).
    const STREAM_LEN: usize = 44;

    /// A symbolic stream of symbolic length ≤ `STREAM_LEN`.
    fn any_stream() -> ([u8; STREAM_LEN], usize) {
        let bytes: [u8; STREAM_LEN] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= STREAM_LEN);
        (bytes, len)
    }

    /// `replay_journal` terminates on every bounded stream and honors
    /// the classification contract: `prefix_len <= len`, `CleanEnd`
    /// iff the whole input verified, verified seqs contiguous from 0,
    /// and the verified prefix replays to `CleanEnd`.
    #[kani::proof]
    #[kani::unwind(64)]
    fn replay_journal_contract_bounded() {
        let (bytes, len) = any_stream();
        let out = replay_journal(&bytes[..len]);
        assert!(out.prefix_len <= len);
        assert!((out.stop == StopReason::CleanEnd) == (out.prefix_len == len));
        for (i, rec) in out.records.iter().enumerate() {
            assert!(rec.seq == i as u64);
        }
        let again = replay_journal(&bytes[..out.prefix_len]);
        assert!(again.stop == StopReason::CleanEnd);
    }

    /// `encode_record` → `replay_journal` is the identity for the two
    /// fixed-size record kinds (Checkpoint, Finalize) over symbolic
    /// fields. The Chunk path (`Vec<JournalSpan>` grow) is excluded —
    /// a symbolic span count forces CBMC through `Vec` allocation on a
    /// symbolic bound; it is covered by the proptest suite and the
    /// concrete cap-boundary tests instead.
    #[kani::proof]
    #[kani::unwind(64)]
    fn encode_replay_roundtrip_bounded() {
        let is_checkpoint: bool = kani::any();
        let payload = if is_checkpoint {
            RecordPayload::Checkpoint(CheckpointRecord {
                checkpoint_index: kani::any(),
                media_len: kani::any(),
                source_frames: kani::any(),
                digest_fold: kani::any(),
                chunk_records: kani::any(),
            })
        } else {
            RecordPayload::Finalize(FinalizeRecord {
                media_len: kani::any(),
                source_frames: kani::any(),
                digest_fold: kani::any(),
                outcome: if kani::any() {
                    FinalizeOutcome::Partial
                } else {
                    FinalizeOutcome::Complete
                },
            })
        };
        let Some(frame) = encode_record(0, &payload) else {
            unreachable!("in-cap payload must encode")
        };
        let out = replay_journal(&frame);
        assert!(out.stop == StopReason::CleanEnd);
        assert!(out.records.len() == 1);
        assert!(out.records[0].seq == 0);
        assert!(out.records[0].payload == payload);
    }

    /// Incremental `Crc32` equals one-shot `crc32` at every split
    /// point of a bounded input.
    #[kani::proof]
    #[kani::unwind(24)]
    fn crc32_split_equivalence_bounded() {
        let bytes: [u8; 16] = kani::any();
        let split: usize = kani::any();
        kani::assume(split <= 16);
        let mut c = Crc32::new();
        c.update(&bytes[..split]);
        c.update(&bytes[split..]);
        assert!(c.finish() == crc32(&bytes));
    }

    /// The session state machine is verified exhaustively: all 16
    /// concrete (from, to) pairs — `legal_transition` agrees with
    /// `require_transition`, legality holds exactly on the three
    /// forward edges, and the acceptance gates hold exactly on their
    /// documented state sets.
    #[kani::proof]
    #[kani::unwind(8)]
    fn session_transitions_exhaustive() {
        let states = [
            SessionState::Created,
            SessionState::Open,
            SessionState::Closed,
            SessionState::Finalized,
        ];
        for &from in &states {
            for &to in &states {
                let legal = legal_transition(from, to);
                assert!(legal == require_transition(from, to).is_ok());
                let forward_edge = matches!(
                    (from, to),
                    (SessionState::Created, SessionState::Open)
                        | (SessionState::Open, SessionState::Closed)
                        | (SessionState::Closed, SessionState::Finalized)
                );
                assert!(legal == forward_edge);
            }
            assert!(accepts_registration(from) == (from == SessionState::Open));
            assert!(
                accepts_take_verification(from)
                    == matches!(from, SessionState::Open | SessionState::Closed)
            );
        }
    }
}
