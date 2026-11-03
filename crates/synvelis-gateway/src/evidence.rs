//! GW-10 evidence schema, sink, and gate aggregation (test/PoC only).
//!
//! Every GW-10 verification case emits one [`EvidenceRecord`] per run:
//! the requirement ids it proves, the fixture, git revision, platform,
//! backend/transport, mock-vs-real, fault-injection point, expected
//! vs actual, a verdict, and the causal trace behind the verdict
//! (Issue #62 / GTV-008).
//!
//! Records append to `<evidence-dir>/<suite>.jsonl` — one file per
//! suite per process — and the `gw10-report` binary folds them into
//! per-case and per-gate verdicts. `NotRun`/`Unsupported` are
//! first-class: a gate only reads `Pass` when every *required* case
//! under it genuinely passed. Mock evidence never stands in for real
//! storage/transport coverage — the `mode` field keeps the two apart.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::trace::TraceEvent;

/// The case verdict — `Unsupported`/`NotRun` are recorded, never
/// silently promoted to `Pass`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Expected and actual agree.
    Pass,
    /// They disagree — the invariant broke.
    Fail,
    /// A prerequisite is missing; the case could not run.
    Blocked,
    /// Nothing attempted the case.
    NotRun,
    /// The platform/harness cannot execute it (e.g. power-loss).
    Unsupported,
}

/// The G0..G6 gate a case feeds (GTV-008 gate vocabulary).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Gate {
    /// Contract: capability/schema/typed-id/result/error.
    G0,
    /// Scope + disclosure: SEC/DATA matrix.
    G1,
    /// Proposal + effect: changeset/digest/approval binding.
    G2,
    /// 3-mode apply + commit/revocation/recovery.
    G3,
    /// Real MCP transport E2E.
    G4,
    /// Job/media/provenance.
    G5,
    /// Cross-cutting system regression (SYS/RT/UX/consumer).
    G6,
    /// Harness self-verification (GTV criteria).
    Gtv,
}

impl Gate {
    /// The wire/serde name.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::G0 => "G0",
            Self::G1 => "G1",
            Self::G2 => "G2",
            Self::G3 => "G3",
            Self::G4 => "G4",
            Self::G5 => "G5",
            Self::G6 => "G6",
            Self::Gtv => "GTV",
        }
    }
}

/// The five-layer split (GTV-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Layer {
    /// Unit/contract/property.
    L1,
    /// Component integration with controlled doubles.
    L2,
    /// In-process real `ProjectController` + mock agent.
    L3,
    /// Real process/IPC/persistence + crash/restart.
    L4,
    /// DAW/RT/real-device/cross-consumer.
    L5,
}

impl Layer {
    /// The wire/serde name.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
            Self::L4 => "L4",
            Self::L5 => "L5",
        }
    }
}

/// Whether the run used controlled doubles or the real surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    /// Controlled doubles / simulated fault seams.
    Mock,
    /// The real component (store, transport, process).
    Real,
}

/// Named fault-injection points (the `fault_point` vocabulary; `NONE`
/// means no injection — the case is a control path).
pub mod fault {
    /// No fault injected — control path.
    pub const NONE: &str = "none";
    /// Pinned-snapshot acquisition boundary.
    pub const SNAPSHOT_ACQUIRE: &str = "snapshot-acquire";
    /// After preflight policy evaluation.
    pub const POST_PREFLIGHT: &str = "post-preflight";
    /// After derived-effect evaluation.
    pub const POST_EFFECT_EVAL: &str = "post-effect-eval";
    /// After approval verification inside the gate.
    pub const POST_APPROVAL_CHECK: &str = "post-approval-check";
    /// At commit-gate entry (scheduled ordering).
    pub const PRE_COMMIT_GATE: &str = "pre-commit-gate";
    /// After final authorization, before project mutation.
    pub const AFTER_AUTHORIZE: &str = "after-authorize";
    /// After in-memory mutation, before the durable record.
    pub const AFTER_APPLY: &str = "after-apply";
    /// After the durable record synced, before settle.
    pub const AFTER_JOURNAL: &str = "after-journal";
    /// Scheduled revoke/commit ordering point.
    pub const SCHEDULED_ORDERING: &str = "scheduled-ordering";
    /// Torn tail write on the framed journal.
    pub const TORN_JOURNAL_TAIL: &str = "torn-journal-tail";
    /// Append failure inside `FileJournal`.
    pub const JOURNAL_APPEND_FAILURE: &str = "journal-append-failure";
    /// Save/HEAD publication boundary.
    pub const SAVE_HEAD_PUBLISH: &str = "save-head-publish";
    /// Process death (kill-equivalent — not power loss).
    pub const PROCESS_KILL: &str = "process-kill";
    /// OS/power loss — always `Unsupported` in this harness.
    pub const POWER_LOSS: &str = "power-loss";
    /// External provider unreachable / non-responsive.
    pub const PROVIDER_UNREACHABLE: &str = "provider-unreachable";
}

/// One emitted evidence record — the GTV-008 schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    /// The matrix case id (e.g. `"SEC-04"`, `"E2E-2-suggest"`).
    pub test_id: String,
    /// Requirement ids the case evidences.
    pub requirements: Vec<String>,
    /// The owning logical unit (`"GW10-A"`..`"GW10-H"`).
    pub owner: String,
    /// The gate (`"G0"`..`"G6"`, `"GTV"`).
    pub gate: String,
    /// The layer (`"L1"`..`"L5"`).
    pub layer: String,
    /// The fixture name.
    pub fixture: String,
    /// Git revision the evidence ran against.
    pub git_revision: String,
    /// `os-arch` the evidence ran on.
    pub platform: String,
    /// Backend (`"in-process"`, `"file-journal"`, `"bundle-store"`...).
    pub backend: String,
    /// Transport (`"in-process"`, `"stdio"`, `"tcp-loopback"`...).
    pub transport: String,
    /// `"mock"` or `"real"`.
    pub mode: String,
    /// The fault-injection point (`fault::*`), `""` for control paths.
    pub fault_point: String,
    /// The expected decision/outcome (oracle-derived).
    pub expected: String,
    /// The observed decision/outcome.
    pub actual: String,
    /// The verdict.
    pub verdict: Verdict,
    /// Causal events backing the verdict.
    pub trace: Vec<TraceEvent>,
    /// Free-form log lines.
    pub log: Vec<String>,
}

/// A matrix row — the canonical case→gate/owner/required mapping.
/// `required = false` rows report honestly but never hold a gate open
/// (features not yet shipped, real-machine/power-loss runs, or
/// cross-repository regressions this repo cannot execute alone).
/// `real_only = true` rows only aggregate records with
/// `mode = "real"` — mock-layer records report but can never satisfy
/// the row, so a mock `Pass` cannot stand in for real-transport
/// coverage (GTV-003: Mockの`Pass`を実環境の`Pass`と取り違えない).
pub struct MatrixRow {
    /// The case id tests emit under.
    pub case: &'static str,
    /// The gate the case feeds.
    pub gate: Gate,
    /// The owning GW10 unit.
    pub owner: &'static str,
    /// Requirement ids the case maps to.
    pub requirements: &'static [&'static str],
    /// Whether the case must Pass for its gate to Pass.
    pub required: bool,
    /// Whether only `mode = "real"` records count toward the verdict.
    pub real_only: bool,
}

/// The GW-10 case matrix — failure-matrix cases plus the GT/AC units
/// they ride on, plus the three E2E paths and the GTV criteria.
pub const MATRIX: &[MatrixRow] = &[
    // ── G0 · GW10-A contract ────────────────────────────────
    row("GT-001", Gate::G0, "GW10-A", &["GT-001", "GTV-001"], true),
    row("GT-002", Gate::G0, "GW10-A", &["GT-002", "GTV-001"], true),
    row("GT-003", Gate::G0, "GW10-A", &["GT-003", "GTV-001"], true),
    // ── G1 · GW10-B scope + disclosure ──────────────────────
    row("SEC-01", Gate::G1, "GW10-B", &["SEC-01"], true),
    row("SEC-02", Gate::G1, "GW10-B", &["SEC-02"], true),
    row("SEC-03", Gate::G1, "GW10-B", &["SEC-03"], true),
    row("SEC-04", Gate::G1, "GW10-B", &["SEC-04"], true),
    row("SEC-05", Gate::G1, "GW10-B", &["SEC-05"], true),
    row("SEC-06", Gate::G1, "GW10-B", &["SEC-06"], true),
    row("SEC-07", Gate::G1, "GW10-B", &["SEC-07"], true),
    row("SEC-08", Gate::G1, "GW10-B", &["SEC-08"], true),
    row("SEC-09", Gate::G1, "GW10-B", &["SEC-09"], true),
    row("SEC-10", Gate::G1, "GW10-B", &["SEC-10"], true),
    row("SEC-11", Gate::G1, "GW10-B", &["SEC-11"], true),
    row("SEC-12", Gate::G1, "GW10-B", &["SEC-12"], true),
    row("SEC-13", Gate::G1, "GW10-B", &["SEC-13"], true),
    row("SEC-14", Gate::G1, "GW10-B", &["SEC-14"], true),
    row("DATA-01", Gate::G1, "GW10-B", &["DATA-01"], true),
    row("DATA-02", Gate::G1, "GW10-B", &["DATA-02"], true),
    row("DATA-03", Gate::G1, "GW10-B", &["DATA-03"], true),
    row("DATA-04", Gate::G1, "GW10-B", &["DATA-04"], true),
    row("DATA-05", Gate::G1, "GW10-B", &["DATA-05"], true),
    row("DATA-06", Gate::G1, "GW10-B", &["DATA-06", "GTV-006"], true),
    row("DATA-07", Gate::G1, "GW10-B", &["DATA-07"], true),
    row("DATA-08", Gate::G1, "GW10-B", &["DATA-08"], true),
    row("DATA-09", Gate::G1, "GW10-B", &["DATA-09"], true),
    row("DATA-10", Gate::G1, "GW10-B", &["DATA-10"], true),
    row("GT-004", Gate::G1, "GW10-B", &["GT-004"], true),
    // ── G2 · GW10-C proposal + effect ───────────────────────
    row("GT-005", Gate::G2, "GW10-C", &["GT-005"], true),
    row("GT-006", Gate::G2, "GW10-C", &["GT-006"], true),
    row("GT-007", Gate::G2, "GW10-C", &["GT-007"], true),
    row("GT-008", Gate::G2, "GW10-C", &["GT-008"], true),
    row("GT-009", Gate::G2, "GW10-C", &["GT-009"], true),
    // ── G3 · commit / revocation / recovery ─────────────────
    row("AC-01", Gate::G3, "GW10-D", &["AC-01"], true),
    row("AC-02", Gate::G3, "GW10-D", &["AC-02"], true),
    row("AC-03", Gate::G3, "GW10-D", &["AC-03"], true),
    row("AC-04", Gate::G3, "GW10-D", &["AC-04"], true),
    row("AC-05", Gate::G3, "GW10-D", &["AC-05"], true),
    row("AC-06", Gate::G3, "GW10-D", &["AC-06"], true),
    row("AC-07", Gate::G3, "GW10-D", &["AC-07"], true),
    row("AC-08", Gate::G3, "GW10-D", &["AC-08"], true),
    row("AC-09", Gate::G3, "GW10-D", &["AC-09"], true),
    row("AC-10", Gate::G3, "GW10-D", &["AC-10"], true),
    row("REC-01", Gate::G3, "GW10-D", &["REC-01", "GTV-005"], true),
    row("REC-02", Gate::G3, "GW10-D", &["REC-02", "GTV-005"], true),
    row("REC-03", Gate::G3, "GW10-D", &["REC-03"], true),
    row("REC-04", Gate::G3, "GW10-D", &["REC-04"], true),
    row("REC-05", Gate::G3, "GW10-D", &["REC-05"], true),
    row("REC-06", Gate::G3, "GW10-D", &["REC-06"], true),
    row("REC-07", Gate::G3, "GW10-E", &["REC-07"], true),
    row("REC-08", Gate::G3, "GW10-E", &["REC-08"], true),
    row("REC-09", Gate::G3, "GW10-E", &["REC-09"], true),
    row("REC-10", Gate::G3, "GW10-E", &["REC-10"], true),
    row("REC-11", Gate::G3, "GW10-E", &["REC-11"], true),
    row("REC-12", Gate::G3, "GW10-E", &["REC-12"], true),
    row("GT-010", Gate::G3, "GW10-D", &["GT-010"], true),
    // ── G4 · real MCP transport E2E ─────────────────────────
    // These rows are `real_only`: an in-process record can add
    // context but only a `mode = "real"` record satisfies them.
    row_real("E2E-2-suggest", Gate::G4, "GW10-F", &["GTV-004"], true),
    row_real("E2E-2-approve", Gate::G4, "GW10-F", &["GTV-004"], true),
    row_real("E2E-2-delegated", Gate::G4, "GW10-F", &["GTV-004"], true),
    row_real(
        "E2E-2-restart",
        Gate::G4,
        "GW10-F",
        &["GTV-004", "GTV-005"],
        true,
    ),
    // ── G5 · job / media / provenance ───────────────────────
    row("JOB-01", Gate::G5, "GW10-G", &["JOB-01"], true),
    row("JOB-02", Gate::G5, "GW10-G", &["JOB-02"], true),
    row("JOB-03", Gate::G5, "GW10-G", &["JOB-03"], true),
    row("JOB-04", Gate::G5, "GW10-G", &["JOB-04"], true),
    row("JOB-05", Gate::G5, "GW10-G", &["JOB-05"], true),
    row("JOB-06", Gate::G5, "GW10-G", &["JOB-06"], true),
    row("JOB-07", Gate::G5, "GW10-G", &["JOB-07"], true),
    row("JOB-08", Gate::G5, "GW10-G", &["JOB-08"], true),
    row("JOB-09", Gate::G5, "GW10-G", &["JOB-09"], true),
    row("JOB-10", Gate::G5, "GW10-G", &["JOB-10"], true),
    row("E2E-3", Gate::G5, "GW10-G", &["GTV-004"], true),
    // ── G6 · cross-cutting system ───────────────────────────
    row("E2E-1", Gate::G6, "GW10-H", &["GTV-004", "GT-012"], true),
    row("SYS-01", Gate::G6, "GW10-H", &["SYS-01", "GT-011"], true),
    row("SYS-02", Gate::G6, "GW10-H", &["SYS-02"], true),
    row("SYS-03", Gate::G6, "GW10-H", &["SYS-03"], true),
    row("SYS-04", Gate::G6, "GW10-H", &["SYS-04"], true),
    row("SYS-05", Gate::G6, "GW10-H", &["SYS-05"], true),
    row("SYS-06", Gate::G6, "GW10-H", &["SYS-06"], true),
    row("SYS-07", Gate::G6, "GW10-H", &["SYS-07", "GTV-007"], true),
    row("SYS-08", Gate::G6, "GW10-H", &["SYS-08"], true),
    // SYS-09 is a process-restart case — real records only.
    row_real("SYS-09", Gate::G6, "GW10-H", &["SYS-09"], true),
    row("SYS-10", Gate::G6, "GW10-H", &["SYS-10"], true),
    row("SYS-02-ux", Gate::G6, "GW10-H", &["SYS-02"], false),
    row("SYS-03-ux", Gate::G6, "GW10-H", &["SYS-03"], false),
    row("SYS-05-powerloss", Gate::G6, "GW10-H", &["SYS-05"], false),
    row(
        "SYS-07-rt",
        Gate::G6,
        "GW10-H",
        &["SYS-07", "GTV-007"],
        false,
    ),
    row("CONSUMER-RUVIFORM", Gate::G6, "GW10-H", &["GTV-001"], false),
    row(
        "CONSUMER-AUDIOLENS",
        Gate::G6,
        "GW10-H",
        &["GTV-001"],
        false,
    ),
    // ── GTV · harness criteria ──────────────────────────────
    row("GTV-001", Gate::Gtv, "GW10-H", &["GTV-001"], true),
    row("GTV-002", Gate::Gtv, "GW10-E", &["GTV-002"], true),
    row("GTV-003", Gate::Gtv, "GW10-D", &["GTV-003"], true),
    row("GTV-004", Gate::Gtv, "GW10-H", &["GTV-004"], true),
    row("GTV-005", Gate::Gtv, "GW10-D", &["GTV-005"], true),
    row("GTV-006", Gate::Gtv, "GW10-F", &["GTV-006"], true),
    row("GTV-007", Gate::Gtv, "GW10-H", &["GTV-007"], true),
    row("GTV-008", Gate::Gtv, "GW10-H", &["GTV-008"], true),
];

const fn row(
    case: &'static str,
    gate: Gate,
    owner: &'static str,
    requirements: &'static [&'static str],
    required: bool,
) -> MatrixRow {
    MatrixRow {
        case,
        gate,
        owner,
        requirements,
        required,
        real_only: false,
    }
}

const fn row_real(
    case: &'static str,
    gate: Gate,
    owner: &'static str,
    requirements: &'static [&'static str],
    required: bool,
) -> MatrixRow {
    MatrixRow {
        case,
        gate,
        owner,
        requirements,
        required,
        real_only: true,
    }
}

/// The matrix row for a case id, when the case is canonical.
#[must_use]
pub fn matrix_row(case: &str) -> Option<&'static MatrixRow> {
    MATRIX.iter().find(|r| r.case == case)
}

/// The evidence output directory: `$SYNVELIS_GW10_EVIDENCE_DIR`, else
/// `$CARGO_TARGET_DIR/gw10-evidence`, else the workspace's
/// `target/gw10-evidence` (resolved from this crate's manifest dir so
/// every suite lands in one sink regardless of the test binary's cwd).
#[must_use]
pub fn evidence_dir() -> PathBuf {
    if let Some(dir) = env::var_os("SYNVELIS_GW10_EVIDENCE_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(dir) = env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(dir).join("gw10-evidence");
    }
    // crates/synvelis-gateway → workspace root → target.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/gw10-evidence")
}

/// The git revision records attribute to — `$SYNVELIS_GIT_REV`, else
/// `git rev-parse HEAD` resolved once per process against this
/// crate's manifest dir, else `"unknown"`. Records must be
/// attributable to the code they ran against (GTV-008).
fn git_revision() -> &'static str {
    static REV: OnceLock<String> = OnceLock::new();
    REV.get_or_init(|| {
        if let Ok(rev) = env::var("SYNVELIS_GIT_REV") {
            return rev;
        }
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_owned())
    })
}

/// One suite's JSONL sink. `Evidence::suite` hands out a shared handle
/// per (suite, process) so concurrent tests interleave line-atomically
/// under a mutex.
pub struct Evidence {
    file: Mutex<File>,
}

static SUITES: OnceLock<Mutex<BTreeMap<String, Arc<Evidence>>>> = OnceLock::new();

impl Evidence {
    /// Opens (or reuses) the suite sink under the evidence directory.
    ///
    /// # Panics
    /// The evidence directory cannot be created or the file opened.
    #[must_use]
    pub fn suite(name: &str) -> Arc<Self> {
        let registry = SUITES.get_or_init(|| Mutex::new(BTreeMap::new()));
        let mut map = registry.lock().unwrap();
        if let Some(ev) = map.get(name) {
            return ev.clone();
        }
        let dir = evidence_dir();
        fs::create_dir_all(&dir).expect("evidence dir");
        let path = dir.join(format!("{name}-{}.jsonl", std::process::id()));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("evidence sink");
        let ev = Arc::new(Self {
            file: Mutex::new(file),
        });
        map.insert(name.to_owned(), ev.clone());
        ev
    }

    /// Begins an evidence record for a matrix case. The fixture is set
    /// per case via [`EvidenceCase::fixture`] — never inherited, so a
    /// record can never carry another case's fixture label.
    #[must_use]
    pub fn case(self: &Arc<Self>, test_id: &str) -> EvidenceCase {
        EvidenceCase {
            ev: self.clone(),
            record: EvidenceRecord {
                test_id: test_id.to_owned(),
                requirements: matrix_row(test_id)
                    .map(|r| r.requirements.iter().map(|s| (*s).to_owned()).collect())
                    .unwrap_or_default(),
                owner: matrix_row(test_id).map_or("GW10-?", |r| r.owner).to_owned(),
                gate: matrix_row(test_id)
                    .map_or("?", |r| r.gate.label())
                    .to_owned(),
                layer: "L2".to_owned(),
                fixture: String::new(),
                git_revision: git_revision().to_owned(),
                platform: format!("{}-{}", env::consts::OS, env::consts::ARCH),
                backend: "in-process".to_owned(),
                transport: "in-process".to_owned(),
                mode: "mock".to_owned(),
                fault_point: String::new(),
                expected: String::new(),
                actual: String::new(),
                verdict: Verdict::NotRun,
                trace: Vec::new(),
                log: Vec::new(),
            },
        }
    }

    fn emit(&self, record: &EvidenceRecord) {
        let line = serde_json::to_string(record).expect("record serializes");
        let mut file = self.file.lock().unwrap();
        writeln!(file, "{line}").expect("evidence write");
        file.flush().expect("evidence flush");
    }
}

/// A single in-flight evidence case — fill it, then `verify`/`close`.
pub struct EvidenceCase {
    ev: Arc<Evidence>,
    record: EvidenceRecord,
}

impl EvidenceCase {
    /// Sets layer/run-mode metadata. The gate is not a parameter —
    /// it is derived from [`matrix_row`] so a record can never
    /// declare a gate its case does not feed.
    #[must_use]
    pub fn meta(mut self, layer: Layer, mode: RunMode) -> Self {
        layer.label().clone_into(&mut self.record.layer);
        match mode {
            RunMode::Mock => "mock",
            RunMode::Real => "real",
        }
        .clone_into(&mut self.record.mode);
        self
    }

    /// Sets the fixture name. The owner is not overridable — it is
    /// matrix-derived like the gate, so attribution cannot be lied
    /// about on either axis.
    #[must_use]
    pub fn fixture(mut self, fixture: &str) -> Self {
        fixture.clone_into(&mut self.record.fixture);
        self
    }

    /// Sets backend and transport labels.
    #[must_use]
    pub fn backend(mut self, backend: &str, transport: &str) -> Self {
        backend.clone_into(&mut self.record.backend);
        transport.clone_into(&mut self.record.transport);
        self
    }

    /// Records the fault-injection point (see [`fault`]).
    #[must_use]
    pub fn fault(mut self, point: &str) -> Self {
        point.clone_into(&mut self.record.fault_point);
        self
    }

    /// Records the expected decision (oracle-derived text).
    #[must_use]
    pub fn expects(mut self, expected: &str) -> Self {
        expected.clone_into(&mut self.record.expected);
        self
    }

    /// Records the observed decision.
    #[must_use]
    pub fn observed(mut self, actual: &str) -> Self {
        actual.clone_into(&mut self.record.actual);
        self
    }

    /// Attaches a causal trace.
    #[must_use]
    pub fn trace(mut self, trace: &[TraceEvent]) -> Self {
        self.record.trace = trace.to_vec();
        self
    }

    /// Appends a log line.
    #[must_use]
    pub fn log(mut self, line: &str) -> Self {
        self.record.log.push(line.to_owned());
        self
    }

    /// Writes the record and returns the verdict. `verify` asserts
    /// expected==actual so a `Fail` verdict always coincides with a
    /// test failure.
    ///
    /// # Panics
    /// `expected != actual` — the record is emitted first. Also
    /// panics if the sink's lock is poisoned or the write fails.
    // The verdict is also returned for callers that branch on it, but
    // the emitted record + assert are the contract — discarding is fine.
    #[allow(clippy::must_use_candidate)]
    pub fn verify(mut self, expected: &str, actual: &str) -> Verdict {
        expected.clone_into(&mut self.record.expected);
        actual.clone_into(&mut self.record.actual);
        let verdict = if expected == actual {
            Verdict::Pass
        } else {
            Verdict::Fail
        };
        self.record.verdict = verdict;
        self.ev.emit(&self.record);
        assert_eq!(
            verdict,
            Verdict::Pass,
            "GW-10 case {} failed: expected {expected:?}, observed {actual:?}",
            self.record.test_id
        );
        verdict
    }

    /// Writes a manual verdict (`Blocked`/`NotRun`/`Unsupported`).
    ///
    /// # Panics
    /// `verdict` is `Pass`/`Fail` — those go through `verify`. Also
    /// panics if the sink's lock is poisoned or the write fails.
    #[allow(clippy::must_use_candidate)]
    pub fn close(mut self, verdict: Verdict, expected: &str, actual: &str) -> Verdict {
        assert!(
            matches!(
                verdict,
                Verdict::Blocked | Verdict::NotRun | Verdict::Unsupported
            ),
            "close() is for non-terminal verdicts; use verify() for Pass/Fail"
        );
        expected.clone_into(&mut self.record.expected);
        actual.clone_into(&mut self.record.actual);
        self.record.verdict = verdict;
        self.ev.emit(&self.record);
        verdict
    }
}

/// Aggregated gate report — the `gw10-report` binary's data.
#[derive(Debug, Default)]
pub struct GateReport {
    /// Per-case aggregate verdict (case id → verdict).
    pub cases: BTreeMap<String, Verdict>,
    /// Per-case records that fed the aggregate.
    pub records: BTreeMap<String, Vec<EvidenceRecord>>,
    /// Per-gate aggregate verdict (gate label → verdict).
    pub gates: BTreeMap<String, Verdict>,
    /// Cases with no evidence at all.
    pub missing: Vec<String>,
}

/// Worse-than ordering for aggregation: a Fail anywhere fails the
/// case; otherwise a Pass anywhere passes it; the uncertainty classes
/// surface only when nothing firmer exists.
fn aggregate(records: &[EvidenceRecord]) -> Option<Verdict> {
    if records.is_empty() {
        return None;
    }
    if records.iter().any(|r| r.verdict == Verdict::Fail) {
        return Some(Verdict::Fail);
    }
    if records.iter().any(|r| r.verdict == Verdict::Pass) {
        return Some(Verdict::Pass);
    }
    records.iter().map(|r| r.verdict).max()
}

/// Reads every `*.jsonl` under `dir` and aggregates case + gate
/// verdicts against [`MATRIX`].
///
/// Gate verdict: `Fail` if any required case failed; `NotRun` if any
/// required case lacks evidence; `Blocked` if any required case is
/// blocked; `Unsupported` if any required case is unsupported; else
/// `Pass`. Non-required rows report but never hold a gate.
#[must_use]
pub fn scan(dir: &Path) -> GateReport {
    let mut report = GateReport::default();
    let Ok(entries) = fs::read_dir(dir) else {
        report.missing = MATRIX.iter().map(|r| r.case.to_owned()).collect();
        for row in MATRIX {
            report.cases.insert(row.case.to_owned(), Verdict::NotRun);
        }
        for gate in [
            Gate::G0,
            Gate::G1,
            Gate::G2,
            Gate::G3,
            Gate::G4,
            Gate::G5,
            Gate::G6,
            Gate::Gtv,
        ] {
            report
                .gates
                .insert(gate.label().to_owned(), Verdict::NotRun);
        }
        return report;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(file) = File::open(&path) else {
            continue;
        };
        for line in BufReader::new(file).lines() {
            let Ok(line) = line else { continue };
            if line.trim().is_empty() {
                continue;
            }
            let Ok(record) = serde_json::from_str::<EvidenceRecord>(&line) else {
                continue;
            };
            report
                .records
                .entry(record.test_id.clone())
                .or_default()
                .push(record);
        }
    }
    for row in MATRIX {
        let all = report.records.get(row.case).cloned().unwrap_or_default();
        // `real_only` rows count only real-mode records toward the
        // verdict — mock records still land in `report.records` for
        // the table, but can never satisfy the row.
        let records: Vec<_> = if row.real_only {
            all.iter().filter(|r| r.mode == "real").cloned().collect()
        } else {
            all
        };
        let verdict = aggregate(&records).unwrap_or(Verdict::NotRun);
        if records.is_empty() {
            report.missing.push(row.case.to_owned());
        }
        report.cases.insert(row.case.to_owned(), verdict);
    }
    for gate in [
        Gate::G0,
        Gate::G1,
        Gate::G2,
        Gate::G3,
        Gate::G4,
        Gate::G5,
        Gate::G6,
        Gate::Gtv,
    ] {
        let mut verdict = Verdict::Pass;
        for row in MATRIX.iter().filter(|r| r.gate == gate && r.required) {
            match report
                .cases
                .get(row.case)
                .copied()
                .unwrap_or(Verdict::NotRun)
            {
                Verdict::Fail => verdict = Verdict::Fail,
                Verdict::NotRun if verdict != Verdict::Fail => verdict = Verdict::NotRun,
                Verdict::Blocked if !matches!(verdict, Verdict::Fail | Verdict::NotRun) => {
                    verdict = Verdict::Blocked;
                }
                Verdict::Unsupported
                    if !matches!(verdict, Verdict::Fail | Verdict::NotRun | Verdict::Blocked) =>
                {
                    verdict = Verdict::Unsupported;
                }
                _ => {}
            }
        }
        report.gates.insert(gate.label().to_owned(), verdict);
    }
    report
}
