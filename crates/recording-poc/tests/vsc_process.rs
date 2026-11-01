//! VS-C 受け入れ: 真の process 終了（crash）→ 別 process で reopen +
//! recover_finalize。in-process の fault 注入ではなく、実 process 間で
//! staging が継続されることを検証する。

use std::{path::PathBuf, process::Command};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_recording-poc"))
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("synvelis-poc-vsc-{}-{name}", std::process::id()));
    if d.exists() {
        std::fs::remove_dir_all(&d).unwrap();
    }
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn vsc_process_kill_then_recover() {
    let root = tmp("crash");
    let session = "77";
    // 1) record --crash: checkpoint 後に process::exit(3)。
    let out = Command::new(bin())
        .args(["vsc", "record", root.to_str().unwrap(), session, "--crash"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(3),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // staging media/journal は残り、finalized.json は無い。
    let tag = synvelis_project::RecordingSessionId::new(77).to_tagged();
    let session_dir = root.join(&tag);
    assert!(session_dir.join("streams/stream-0000.pcm").exists());
    assert!(session_dir.join("streams/stream-0000.journal").exists());
    assert!(
        !session_dir
            .join("streams/stream-0000.finalized.json")
            .exists()
    );
    let media_len = std::fs::metadata(session_dir.join("streams/stream-0000.pcm"))
        .unwrap()
        .len();
    assert!(media_len > 0);

    // 2) recover: Last Valid Checkpoint まで Partial ref を公開。
    let out = Command::new(bin())
        .args(["vsc", "recover", root.to_str().unwrap(), session])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("vsc_recover stream=0 outcome=Partial frames=48000"),
        "{stdout}"
    );
    assert!(
        session_dir
            .join("streams/stream-0000.finalized.json")
            .exists()
    );

    // 3) 再 recover は AlreadyFinalized（二重 finalize しない・成功終了）。
    let out = Command::new(bin())
        .args(["vsc", "recover", root.to_str().unwrap(), session])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("already_finalized"));

    // 4) finalized ref の media_byte_len は crash 時の media 長と一致。
    let fin: synvelis_recording::FinalizedSourceRef = serde_json::from_slice(
        &std::fs::read(session_dir.join("streams/stream-0000.finalized.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(fin.media_byte_len, media_len);
    assert_eq!(fin.source_frames, 48_000);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn vsc_process_record_finalize_normal() {
    let root = tmp("normal");
    let out = Command::new(bin())
        .args(["vsc", "record", root.to_str().unwrap(), "78"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("vsc_finalize stream=0 outcome=Complete frames=48000"),
        "{stdout}"
    );
    std::fs::remove_dir_all(&root).ok();
}
