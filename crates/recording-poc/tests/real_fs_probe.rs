//! RCD-02 (synvelis#116) real-FS probe: 実 FS 上の rename 原子性・
//! dir sync capability・unsynced tail の実挙動を opt-in で実測する。
//!
//! 全 test が `#[ignore]` で、既定 `cargo test` / CI には含まれない
//! （`mise run test-recording-real-fs`）。観測値は stdout に出し、
//! `evidence/reports/rcd-02-durability-recovery-review-ledger.md`
//! へ実測行として転記する。真の power-loss は非破壊的に自動化
//! できないため対象外 — process kill（TerminateProcess 相当）で
//! 代替可能な範囲のみを実測し、power-loss 相当の Pass は主張
//! しない。

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_recording-poc"))
}

fn tmp(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("synvelis-poc-realfs-{}-{name}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn probe_map(dir: &Path) -> BTreeMap<String, String> {
    let out = Command::new(bin())
        .args(["realfs", "probe", dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut map = BTreeMap::new();
    for line in stdout.lines() {
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        }
    }
    map
}

/// 実 FS 上の rename/dir sync/publish/file 観測値を print し、
/// 環境非依存の不変条件と Windows 期待値を検証する。
#[test]
#[ignore = "real-FS lane: requires a real local filesystem (run `mise run test-recording-real-fs`)"]
fn realfs_probe_fs_capabilities() {
    let root = tmp("probe");
    let dir = root.join("work");
    let observed = probe_map(&dir);
    for (key, value) in &observed {
        println!("observed {key}={value}");
    }
    let get = |key: &str| observed.get(key).map(String::as_str);

    // publish/rename の不変条件（環境非依存）。
    assert_eq!(get("rename_missing_dest"), Some("ok"));
    assert_eq!(get("rename_replace"), Some("ok"));
    assert_eq!(get("rename_replace_content_ok"), Some("true"));
    assert_eq!(get("create_new_existing"), Some("err:AlreadyExists"));
    assert_eq!(get("publish_atomic_replace"), Some("ok"));
    assert_eq!(get("publish_atomic_content_ok"), Some("true"));
    assert_eq!(get("publish_atomic_create_new"), Some("ok"));
    assert_eq!(
        get("publish_create_new_conflict"),
        Some("err:AlreadyExists")
    );
    assert_eq!(get("publish_create_new_content_kept"), Some("true"));
    assert_eq!(get("publish_residue"), Some("0"));
    assert_eq!(get("file_nosync_len"), Some("4096"));
    assert_eq!(get("set_len"), Some("1024"));

    // dir sync: raw `File::open(dir)`/`sync_all` 観測と detector
    // 分類が整合し、非対応を `Supported` に偽装しない。
    let dir_ok = get("dir_open") == Some("ok") && get("dir_sync") == Some("ok");
    let detected = get("detect_dir_sync").unwrap_or("");
    if !dir_ok {
        assert_ne!(
            detected, "Supported",
            "detect_dir_sync must not claim Supported when raw dir sync fails"
        );
    }
    #[cfg(windows)]
    {
        // Windows/NTFS: dir の File::open/sync は非対応という
        // 観測を pin する（manifest 記録値と同一の detector）。
        assert_eq!(detected, "Unsupported");
    }
    // rename_src_open/rename_dst_open/dir_* の値自体は環境差と
    // して記録する（Rust std は FILE_SHARE_DELETE を付与するため
    // open file の rename も成功し得る — 観測値が正本）。
    std::fs::remove_dir_all(&root).ok();
}

/// `write-unsynced` 子 process を ready 検出後に kill し、
/// unsynced tail が post-kill で観測できることを実測する。
/// process kill は dirty page を失わせない（= power-loss とは
/// 別物）という実測であり、power-loss 行を `NotRun` に留める
/// 根拠になる。
#[test]
#[ignore = "real-FS lane: requires a real local filesystem (run `mise run test-recording-real-fs`)"]
fn realfs_unsynced_tail_survives_process_kill() {
    let root = tmp("kill");
    let file = root.join("unsynced.bin");
    let synced = 1024usize;
    let unsynced = 2048usize;
    let mut child = Command::new(bin())
        .args([
            "realfs",
            "write-unsynced",
            file.to_str().unwrap(),
            &synced.to_string(),
            &unsynced.to_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    // ready 行で同期（sleep に頼らない）。
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    let ready = loop {
        match lines.next() {
            Some(Ok(line)) if line.contains("ready") => break line,
            Some(Ok(_)) => {}
            other => panic!("child exited before ready: {other:?}"),
        }
    };
    println!("child: {ready}");
    // graceful close ではなく TerminateProcess 相当で殺す。
    child.kill().unwrap();
    let status = child.wait().unwrap();
    println!("killed status: {status:?}");

    // 別 process（本 test process）から観測: sync 未達の tail
    // も file に見える — process kill は OS の dirty page を
    // 失わせず、unsynced 喪失を再現しない。
    let bytes = std::fs::read(&file).unwrap();
    println!(
        "observed post-kill len={} synced={synced} unsynced={unsynced}",
        bytes.len()
    );
    assert!(
        bytes.len() >= synced,
        "synced prefix must be observable after kill"
    );
    assert_eq!(
        bytes.len(),
        synced + unsynced,
        "process kill preserves dirty pages — unsynced tail still observable"
    );
    assert!(bytes[..synced].iter().all(|&byte| byte == 0xA5));
    assert!(bytes[synced..].iter().all(|&byte| byte == 0x5A));
    println!("post-kill content verified: synced prefix + unsynced tail intact");
    std::fs::remove_dir_all(&root).ok();
}
