//! RCD-02 task3: staging publish の ruviform `StagedFile` 移行を
//! 検証する focused test。`publish_atomic`/`publish_atomic_faultable`/
//! `publish_atomic_create_new` の公開契約（Replace/CreateNew、
//! Denied/SyncFail/Indeterminate seam、temp cleanup、call 計数）を
//! 実 FS 上で確認する。実 OS power-loss durability の証明ではない
//! （fault-matrix test-spec の Non-goals 参照）。

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use synvelis_recording::{
    PublishFaults, PublishInject, publish_atomic, publish_atomic_create_new,
    publish_atomic_faultable,
};

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-staged-publish-{}-{}",
        std::process::id(),
        name
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// dir 内の file 名一覧（temp residue 検査用）。
fn entries(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn publish_atomic_replace_overwrites_existing() {
    let dir = tmp_dir("replace");
    let path = dir.join("artifact.bin");
    publish_atomic(&path, b"first").unwrap();
    publish_atomic(&path, b"second-longer").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"second-longer");
    // temp residue なし。
    assert_eq!(entries(&dir), vec!["artifact.bin"]);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn publish_atomic_create_new_refuses_existing() {
    let dir = tmp_dir("create-new");
    let path = dir.join("manifest.json");
    publish_atomic_create_new(&path, b"v1").unwrap();
    let err = publish_atomic_create_new(&path, b"v2").unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    // 既存 bytes は atomic no-replace で untouched。
    assert_eq!(fs::read(&path).unwrap(), b"v1");
    assert_eq!(entries(&dir), vec!["manifest.json"]);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn publish_faultable_denied_touches_nothing() {
    let dir = tmp_dir("denied");
    let path = dir.join("review.json");
    let mut faults = PublishFaults::new();
    faults.inject(PublishInject::Denied { call: 0 });
    let err = publish_atomic_faultable(&path, b"data", &mut faults).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert!(!path.exists());
    assert_eq!(entries(&dir), Vec::<String>::new());
    assert_eq!(faults.calls(), 1);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn publish_faultable_sync_fail_leaves_destination_unpublished() {
    let dir = tmp_dir("syncfail");
    let path = dir.join("finalized.json");
    let mut faults = PublishFaults::new();
    faults.inject(PublishInject::SyncFail { call: 0 });
    let err = publish_atomic_faultable(&path, b"data", &mut faults).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::Other);
    assert!(err.to_string().contains("injected publish sync failure"));
    // publish 前に失敗するため destination は未公開、temp も残らない。
    assert!(!path.exists());
    assert_eq!(entries(&dir), Vec::<String>::new());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn publish_faultable_indeterminate_publishes_but_reports_error() {
    let dir = tmp_dir("indeterminate");
    let path = dir.join("commit.marker");
    let mut faults = PublishFaults::new();
    faults.inject(PublishInject::Indeterminate { call: 0 });
    let err = publish_atomic_faultable(&path, b"marker-bytes", &mut faults).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::Other);
    assert!(err.to_string().contains("injected indeterminate publish"));
    // publish は成功済み — destination は新 bytes を持つ（応答喪失相当）。
    assert_eq!(fs::read(&path).unwrap(), b"marker-bytes");
    assert_eq!(entries(&dir), vec!["commit.marker"]);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn publish_faultable_call_counter_only_counts_faultable_calls() {
    let dir = tmp_dir("calls");
    let path = dir.join("f.bin");
    // 素の publish_atomic は faultable の call カウンタを消費しない
    // （PublishFaults を経由しない）。
    publish_atomic(&path, b"plain").unwrap();
    let mut faults = PublishFaults::new();
    faults.inject(PublishInject::Denied { call: 0 });
    // call 0 がまだ Denied に刺さる = publish_atomic は計数しなかった。
    assert!(publish_atomic_faultable(&path, b"x", &mut faults).is_err());
    assert_eq!(faults.calls(), 1);
    // call 1 は注入なし → 通常 publish（Replace）。
    publish_atomic_faultable(&path, b"second", &mut faults).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"second");
    assert_eq!(faults.calls(), 2);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn publish_atomic_reports_stage_failure_without_destination() {
    // 存在しない dir への publish は stage で失敗し、destination は
    // 作られない（`exists()` 先行チェックには依存しない）。
    let dir = tmp_dir("stagefail");
    let missing = dir.join("no-such-dir").join("f.bin");
    let err = publish_atomic(&missing, b"data").unwrap_err();
    assert_ne!(err.kind(), io::ErrorKind::AlreadyExists);
    assert!(!missing.exists());
    fs::remove_dir_all(&dir).ok();
}
