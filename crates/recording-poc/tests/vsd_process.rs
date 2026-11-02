//! VS-D 受け入れ: `recording-poc vsd` の E2E（record → finalize →
//! commit → save/reload → retry → undo/redo → marker/status）が実
//! process で決定的に成功することを検証する。

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_recording-poc"))
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("synvelis-poc-vsd-{}-{name}", std::process::id()));
    if d.exists() {
        std::fs::remove_dir_all(&d).unwrap();
    }
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run_vsd(root: &Path, session: &str) -> String {
    let out = Command::new(bin())
        .args(["vsd", root.to_str().unwrap(), session])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "vsd failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn vsd_process_e2e_deterministic() {
    let root1 = tmp("run1");
    let root2 = tmp("run2");
    let first = run_vsd(&root1, "90");
    let second = run_vsd(&root2, "91");

    // 全出力は決定的（path を含まない・mock input 固定）。
    assert_eq!(first, second, "vsd output must be deterministic");
    assert!(first.contains("result=OK"), "{first}");
    assert!(first.contains("vsd_commit outcome=applied"), "{first}");
    assert!(first.contains("vsd_reload_equal=true"), "{first}");
    assert!(
        first.contains("vsd_retry outcome=already-applied"),
        "{first}"
    );
    assert!(first.contains("vsd_media_preserved=true"), "{first}");

    std::fs::remove_dir_all(&root1).ok();
    std::fs::remove_dir_all(&root2).ok();
}
