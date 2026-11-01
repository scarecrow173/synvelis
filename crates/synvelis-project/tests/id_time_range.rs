//! AC2/AC3/AC4: stable-ID round-trip, checked time arithmetic, `[start,end)`.

use synvelis_project::*;

#[test]
fn all_ids_roundtrip_tagged() {
    macro_rules! check {
        ($t:ty, $kind:literal) => {{
            let id = <$t>::new(0xdead_beef_u128);
            let tagged = id.to_tagged();
            assert!(tagged.starts_with(concat!($kind, "-")), "{tagged}");
            assert_eq!(<$t>::from_tagged(&tagged).unwrap(), id);
            let json = serde_json::to_string(&id).unwrap();
            assert_eq!(json, format!("\"{tagged}\""));
            let back: $t = serde_json::from_str(&json).unwrap();
            assert_eq!(back, id);
        }};
    }
    check!(ProjectId, "project");
    check!(TrackId, "track");
    check!(LaneId, "lane");
    check!(ClipId, "clip");
    check!(SourceId, "source");
    check!(AssetId, "asset");
    check!(TakeId, "take");
    check!(RecordingSessionId, "recording-session");
    check!(RecordingPassId, "recording-pass");
    check!(CaptureStreamId, "capture-stream");
    check!(CaptureSegmentId, "capture-segment");
    check!(ClockEpochId, "clock-epoch");
}

#[test]
fn id_kind_mismatch_rejected() {
    // Serialize a TrackId, attempt to read it as a LaneId.
    let json = serde_json::to_string(&TrackId::new(5)).unwrap();
    assert!(serde_json::from_str::<LaneId>(&json).is_err());
    assert!(TrackId::from_tagged("lane-00000000000000000000000000000005").is_err());
    assert!(TrackId::from_tagged("track-xyz").is_err());
    assert!(TrackId::from_tagged("track-5").is_err()); // must be 32 hex
}

#[test]
fn session_and_source_frames_are_checked_and_non_negative() {
    assert!(SessionFrame::new(-1).is_err());
    assert!(SourceFrame::new(-1).is_err());
    let f = SessionFrame::new(48_000).unwrap();
    assert_eq!(f.checked_add(1).unwrap().value(), 48_001);
    assert!(f.checked_add(i64::MAX).is_err()); // overflow
    assert!(f.checked_sub(48_001).is_err()); // would go negative
    assert!(f.checked_sub(i64::MIN).is_err()); // overflow before negative check
    assert_eq!(
        f.frames_since(SessionFrame::new(47_000).unwrap()).unwrap(),
        1_000
    );
    assert!(f.frames_since(SessionFrame::new(48_001).unwrap()).is_err());
}

#[test]
fn musical_tick_and_project_time_allow_negative() {
    let t = MusicalTick::new(-960);
    assert_eq!(t.checked_add(960).unwrap().value(), 0);
    assert!(MusicalTick::new(i64::MAX).checked_add(1).is_err());
    let pt = ProjectTime::Musical(MusicalTick::new(-1));
    let json = serde_json::to_string(&pt).unwrap();
    let back: ProjectTime = serde_json::from_str(&json).unwrap();
    assert_eq!(back, pt);
    let media = MediaTimeValue::new(-48_000, 1, 48_000).unwrap();
    assert!(MediaTimeValue::new(0, 0, 1).is_err());
    assert!(MediaTimeValue::new(0, 1, 0).is_err());
    let _ = media;
}

#[test]
fn half_open_ranges_validate_and_dont_overlap() {
    let a = SessionRange::try_new(
        SessionFrame::new(0).unwrap(),
        SessionFrame::new(100).unwrap(),
    )
    .unwrap();
    let b = SessionRange::try_new(
        SessionFrame::new(100).unwrap(),
        SessionFrame::new(200).unwrap(),
    )
    .unwrap();
    assert!(!a.overlaps(&b)); // adjacent [0,100) and [100,200): no shared sample
    assert!(a.contains(SessionFrame::new(99).unwrap()));
    assert!(!a.contains(SessionFrame::new(100).unwrap()));
    assert!(a.intersection(&b).is_none());
    let c = SessionRange::try_new(
        SessionFrame::new(50).unwrap(),
        SessionFrame::new(150).unwrap(),
    )
    .unwrap();
    assert!(a.overlaps(&c));
    assert_eq!(
        a.intersection(&c).unwrap(),
        SessionRange::try_new(
            SessionFrame::new(50).unwrap(),
            SessionFrame::new(100).unwrap()
        )
        .unwrap()
    );
    // inverted construction rejected — including via serde
    assert!(
        SessionRange::try_new(SessionFrame::new(5).unwrap(), SessionFrame::new(4).unwrap())
            .is_err()
    );
    let json = r#"{"start":10,"end":4}"#;
    assert!(serde_json::from_str::<SessionRange>(json).is_err());
    // empty range legal at type level
    let empty = SessionRange::try_new(SessionFrame::new(7).unwrap(), SessionFrame::new(7).unwrap())
        .unwrap();
    assert!(empty.is_empty());
    assert!(!empty.contains(SessionFrame::new(7).unwrap()));
}

#[test]
fn media_span_orders_across_bases() {
    let one = MediaTimeValue::new(1_000, 1, 1_000).unwrap(); // 1.0 s
    let half_48k = MediaTimeValue::new(24_000, 1, 48_000).unwrap(); // 0.5 s
    assert!(half_48k.lt(&one));
    assert!(one.ge(&half_48k));
    let span = MediaSpan::try_new(half_48k, one).unwrap();
    assert!(!span.is_empty());
    assert!(MediaSpan::try_new(one, half_48k).is_err());
    let json = serde_json::to_string(&span).unwrap();
    let back: MediaSpan = serde_json::from_str(&json).unwrap();
    assert_eq!(back, span);
}
