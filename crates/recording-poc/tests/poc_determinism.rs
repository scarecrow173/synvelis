//! AC11: the headless PoC produces identical output across runs.

use std::process::Command as Proc;

fn run_poc() -> String {
    let output = Proc::new(env!("CARGO_BIN_EXE_recording-poc"))
        .output()
        .expect("poc must run");
    assert!(
        output.status.success(),
        "poc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn two_runs_produce_identical_output() {
    let first = run_poc();
    let second = run_poc();
    assert_eq!(first, second);
    assert!(first.contains("result=OK"));
    assert!(first.contains("roundtrip_equal=true"));
}
