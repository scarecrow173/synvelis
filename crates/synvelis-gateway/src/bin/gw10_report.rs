//! `gw10-report` — aggregates the GW-10 evidence JSONL into the
//! per-case / per-gate verdict report (GTV-008).
//!
//! Usage: `gw10-report [evidence-dir] [--strict] [--clean]`
//! - `evidence-dir` defaults to `$SYNVELIS_GW10_EVIDENCE_DIR` or
//!   `target/gw10-evidence` (see `evidence::evidence_dir`).
//! - `--clean` empties the evidence directory and exits — the `gw10`
//!   task runs it before the suites so per-process `*-<pid>.jsonl`
//!   files from older runs can never mask a case that stopped
//!   emitting.
//! - `--strict` exits 2 when any required case failed or never ran —
//!   the CI gate hook. Without it the report is informational.

use std::path::PathBuf;
use std::process::ExitCode;

use synvelis_gateway::evidence::{self, MATRIX, Verdict};

fn verdict_label(v: Verdict) -> &'static str {
    match v {
        Verdict::Pass => "PASS",
        Verdict::Fail => "FAIL",
        Verdict::Blocked => "BLOCKED",
        Verdict::NotRun => "NOT-RUN",
        Verdict::Unsupported => "UNSUPPORTED",
    }
}

fn main() -> ExitCode {
    let mut dir: Option<PathBuf> = None;
    let mut strict = false;
    let mut clean = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--strict" => strict = true,
            "--clean" => clean = true,
            other => dir = Some(PathBuf::from(other)),
        }
    }
    let dir = dir.unwrap_or_else(evidence::evidence_dir);
    if clean {
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("evidence dir removes");
        }
        std::fs::create_dir_all(&dir).expect("evidence dir recreates");
        println!("cleaned {}", dir.display());
        return ExitCode::SUCCESS;
    }
    let report = evidence::scan(&dir);
    let dir = dir.canonicalize().unwrap_or(dir);

    println!("gw-10 gate report — {}", dir.display());
    println!("{:<6} {:<8} meaning", "gate", "verdict");
    for (gate, verdict) in &report.gates {
        let meaning = match gate.as_str() {
            "G0" => "GW10-A contract",
            "G1" => "GW10-B authorization + disclosure",
            "G2" => "GW10-C proposal/effect integration",
            "G3" => "GW10-D/E commit, concurrency, recovery",
            "G4" => "GW10-F real MCP transport E2E",
            "G5" => "GW10-G job/media/provenance",
            "G6" => "GW10-H system non-regression",
            "GTV" => "harness self-verification criteria",
            _ => "",
        };
        println!("{gate:<6} {:<8} {meaning}", verdict_label(*verdict));
    }

    println!();
    println!(
        "{:<22} {:<12} {:<8} requirements",
        "case", "owner", "verdict"
    );
    for row in MATRIX {
        let verdict = report
            .cases
            .get(row.case)
            .copied()
            .unwrap_or(Verdict::NotRun);
        let n = report.records.get(row.case).map_or(0, Vec::len);
        println!(
            "{:<22} {:<12} {:<8} {} ({} records)",
            row.case,
            row.owner,
            verdict_label(verdict),
            row.requirements.join(","),
            n
        );
    }

    // Non-matrix records — supplementary evidence that feeds no gate.
    let extra: Vec<_> = report
        .records
        .keys()
        .filter(|k| !MATRIX.iter().any(|r| r.case == k.as_str()))
        .cloned()
        .collect();
    if !extra.is_empty() {
        println!();
        println!(
            "supplementary records (no matrix row): {}",
            extra.join(", ")
        );
    }

    if !report.missing.is_empty() {
        println!();
        println!("missing evidence: {}", report.missing.join(", "));
    }

    // `--strict` gates on every gate being Pass — Fail, NotRun,
    // Blocked, and Unsupported all mean a *required* case did not
    // genuinely pass (the gate loop already ignores optional rows).
    if strict && report.gates.values().any(|v| *v != Verdict::Pass) {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
