# Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 — Evidence Ledger

Integrated acceptance evidence for the `synvelis-player` consumer slice of
`synvelis-library::import_verified`. `Pass` here means *the stated
headless lane only* — it never implies real GUI, real audio device, RT
processing-thread, or app-layer acceptance. Those lanes are
`NotRun`/`Unsupported` with reasons.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis branch | `feature/86-verified-import-wiring` |
| Base commit | `9d6b963` (origin/main merged 2026-11-11, clean merge — no conflicts) |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.0 (b940084d7); MSRV lane rustc 1.89.0 |
| Fixture | temp-dir `ProjectController` + `ImportStore`; `LocalFolderProvider` root-bounded pin; `HeadlessDriver` deterministic WAV |
| CI/PR | see PR body (Refs #86) |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Fail` = measured failure; `NotRun` = lane exists but was not measured
here; `Unsupported` = capability genuinely unavailable on this host.

## Canonical lane (mise tasks, `--locked`)

| Command | Expected | Observed | Status |
| --- | --- | --- | --- |
| `mise run fmt-check` | exit 0 | exit 0 | Pass |
| `mise run check` | exit 0 | exit 0 | Pass |
| `mise run lint` | exit 0 (`-D warnings`) | exit 0 | Pass |
| `mise run test` | exit 0, workspace tests pass | exit 0 | Pass |
| `mise run test-rt` | exit 0 | exit 0 | Pass |
| `mise run test-msrv` | exit 0 (rust 1.89 workspace tests) | exit 0 | Pass |

## Acceptance rows — `crates/synvelis-player/tests/verified_import.rs`

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| VIMP-01 sha256 import→resolve→playback | `import_verified_local` commits `Imported`, entry verifies blob+decode, `SetSourceState`→`Verified`, `resolver::prepare` + `HeadlessDriver` PCM sample match | `sha256_import_resolves_and_plays` Pass | Pass |
| VIMP-02 Imported gating | source committed by `import_verified` alone stays `Imported` and `resolver::prepare` rejects as unverified until entry verifies | `imported_source_is_gated_until_entry_verifies` Pass | Pass |
| VIMP-03 legacy Crc32Fold | 16-char crc-fold digest resolves from flat `blobs/<digest>` and plays (non-regression) | `crc32_fold_source_still_resolves` Pass | Pass |
| VIMP-04 opaque digest | foreign/opaque digest string → `UnsupportedDigest` fail-closed, never probed on disk | `opaque_digest_fails_closed` Pass | Pass |
| VIMP-05 sha256 missing/mismatch | absent blob → `MissingMedia`; corrupted blob → hash-mismatch typed error, no decode | `sha256_missing_and_mismatch_fail_typed` Pass | Pass |
| VIMP-06 undo→re-import reserved IDs | `undo()` then re-import after rebase: manifest reserved IDs converge on the original asset/source/clip identities | `undo_then_reimport_converges_reserved_ids` Pass | Pass |
| VIMP-07 saved HEAD→load→play | `save` + fresh `ProjectController::load` replays verified source end-to-end | `saved_head_load_replays` Pass | Pass |
| VIMP-08 interrupted job inspect→resume | `inspect_imports` finds interrupted job, resume commits, source reaches playable state | `interrupted_job_inspects_resumes_and_plays` Pass | Pass |
| VIMP-09 stale revision + explicit rebase | blind retry after undo → typed `VerifiedImportError::Import(StaleRevision)`; `rebase_import` re-arms and the retry converges on the same reserved IDs | inside `undo_then_reimport_converges_reserved_ids` (lines ~385-421) Pass | Pass |

## Non-regression

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| `import_local` legacy path | untouched; its 20 test call sites keep passing | `mise run test` exit 0 (all `synvelis-player::import` tests green) | Pass |
| `resolver::prepare` existing sources | Crc32Fold sources keep resolving | workspace suite green incl. existing resolver tests | Pass |
| `render::prepare::open_source` | now shares `open_blob_verified`; existing render tests green | `mise run test` exit 0 | Pass |

## NotRun / Unsupported

| Lane | Status | Reason / follow-up |
| --- | --- | --- |
| RT processing thread | NotRun | scope excludes RT; `test-rt` lane Pass covers player RT-adjacent tests only |
| Real audio device | Unsupported | headless-only slice; no device lane on this host |
| Real GUI / app layer | NotRun | app wiring is a separate issue; this slice ends at `synvelis-player` public API |
| DirSync durability | Unsupported | Windows NTFS directory fsync unsupported — recorded per spec, not blocking |
