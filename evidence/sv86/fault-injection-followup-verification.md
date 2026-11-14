# Evidence — synvelis#86 ALIB-02 fault-injection followup: 残存 integrity/dirsync 拒否経路の専用 test + spec AC 照合

Issue #86（ALIB-02: Streaming Verified Import・SHA-256 識別子）の
残存 fault-injection slice。`evidence/sv86/bookkeeping-verification.md`
の AC 突合表が「実装済みだが専用 test 未整備」として未チェック維持と
判定した 2 行（integrity 拒否 4 経路の残り 3 経路 + DirSync
`Unsupported`）を、`crates/synvelis-library/tests/import.rs` の
scripted double による独立 test で実測し、spec AC を実在 test 名と
1:1 で照合した。

`Pass` は「記録された headless/scripted lane の実測のみ」を意味し、
実 OS/FS・実 GUI・実 provider・RT・cross-consumer の受入を示さない。
scripted `BlobPublisher`/`AssetProvider` double は fault *分類* の
証拠であり実 durability の証拠ではない。それらの外部 gate
（synvelis#91/#97/#114/#331・ruviform#294）は `NotRun`/tracking の
まま残し、Issue #86 は open 継続とする。

## Anchor — 対象 SHA・環境

- branch: `feat/86-alib02-fault-tests`（dispatch worktree
  `scarecrow173/sv86-alib02fault`）。base は dispatch 時点の
  `origin/main` `b70e339`（2026-11-15 fetch 済み、HEAD == origin/main）。
- 変更面: `crates/synvelis-library/tests/import.rs`（test のみ）+
  docs/evidence。`src/**`・`Cargo.toml`/`Cargo.lock`・
  `.github/workflows/` の変更ゼロ。
- 実行環境: Windows（headless）、mise 管理ツールチェーン
  （rustc 1.99.0 系、canonical = `mise run`）。
- doc chain: `docs/designs|plans|tasks|impl/ir/
  alib-02-fault-injection-followup.md`（新規）。experiment log
  （`docs/impl/exp/`）は試行錯誤が無かったため未作成 — IR
  `alib-02-fault-injection-followup.md` Decisions に記録。

## 変更内容

1. `crates/synvelis-library/tests/import.rs`（21 → 25 tests）:
   - `size_mismatch_is_integrity_error`（新規）— intent pin の
     `expected_size` を +1 改変して `check_pin_expectations`
     （`src/import.rs` ~L823-844）を踏む。`ImportError::
     IntegrityMismatch`、project 未変更・blob path 非存在・job は
     `ImportStage::Started` 維持・新規 ImportId の誠実な import が
     収束することを assert。
   - `staged_drift_is_corrupt`（新規）— 既存 `CancelOnEofProvider`
     で `Staged` まで進めて cancel → `source.bin` を改変 → 同一
     ImportId retry で `verify_staged`（~L926-956）の readback
     再計測不一致 → `ImportError::Corrupt`。未 commit・blob 未公開・
     manifest `Staged` 維持を assert し、bytes 復元後に同一
     ImportId で収束。
   - `existing_blob_mismatch_is_integrity_error`（新規）—
     正しい digest の `blobs/sha256/<hex>` に不一致 occupant を
     事前配置し `StagedBlobPublisher` 経由で import →
     `verify_existing_blob`（~L959-979）→ `ImportError::
     IntegrityMismatch`。occupant 非上書き・未 commit・manifest
     `Verified` 維持を assert し、occupant 修復後に
     `BlobMaterialization::Reused` で収束。
   - `DirSyncUnsupportedPublisher`（新規 double）+
     `directory_sync_unsupported_is_recorded_not_hidden`（新規）—
     実 publish を `StagedBlobPublisher` に delegate した上で
     receipt の `directory_sync` を `DirectorySync::Unsupported` に
     改変して返す。outcome `blob.directory_sync == Unsupported`・
     manifest `directory_sync == Some(Unsupported)`・stage
     `Committed`・commit 収束を assert。test コメントに
     「分類証拠であり実 durability 証拠ではない」旨を明記。
2. `docs/specs/alib-02-streaming-verified-import.md`:
   - AC「hash mismatch・size mismatch・staged drift・既存 blob
     不一致を `IntegrityMismatch`/`Corrupt` として拒否」を `[x]` 化
     （実測 test 名を行末コメントに記録）。
   - AC「DirSync `Unsupported`/`Failed` が receipt/manifest に記録
     され publish 成否と分離される」を `[x]` 化（`Unsupported` が
     scripted 分類証拠であり実 durability は外部 gate 残存である旨を
     コメントに記録）。
   - `updated` → 2026-11-15。
3. `docs/test-specs/alib-02-streaming-verified-import.md`:
   - test 数記述を 21 → 25 に更新（新規 4 tests の由来を追記）。
   - Idempotency/integrity Feature に新規 3 tests、Publish honesty
     Feature に DirSync test を Example 追記。`updated` → 2026-11-15。
4. doc chain 新規一式 + 本ファイル。

## AC 行 ↔ 実測証拠 突合表（本 slice で閉じた 2 行）

| # | spec AC 行（要約） | 判定 | 根拠（実測 test 名・lane） |
| --- | --- | --- | --- |
| 6 | hash/size/staged drift/既存 blob 不一致を `IntegrityMismatch`/`Corrupt` 拒否 | [x] | hash mismatch: `digest_mismatch_is_integrity_error`（既存）。size mismatch: `size_mismatch_is_integrity_error`（新規、pin `expected_size` 照合 → `IntegrityMismatch`・Started 維持・fresh import 収束）。staged drift: `staged_drift_is_corrupt`（新規、`verify_staged` readback → `Corrupt`・Staged 維持・復元後同一 ImportId 収束）。既存 blob 不一致: `existing_blob_mismatch_is_integrity_error`（新規、`verify_existing_blob` → `IntegrityMismatch`・occupant 非上書き・Verified 維持・修復後 Reused 収束）。`mise run test synvelis-library` で 25/25 Pass |
| 8 | DirSync `Unsupported`/`Failed` が receipt/manifest に記録・publish 成否と分離 | [x] | `Failed`: `directory_sync_failure_is_recorded_not_hidden`（既存、publish 成功と dirsync 失敗の分離記録）。`Unsupported`: `directory_sync_unsupported_is_recorded_not_hidden`（新規、`DirSyncUnsupportedPublisher` が実 publish 後に receipt を `Unsupported` に改変 → outcome・manifest 双方に記録・commit 独立収束）。**scripted 分類証拠であり実 durability 証拠ではない** — 実 OS/FS の DirSync 可否は synvelis#91 の外部 gate |

残る spec AC 行は `bookkeeping-verification.md` の突合表どおり
全て `[x]`（実測済み）であり、本 slice で spec の未チェック行は
ゼロになった。ただし Issue #86 の受入条件 AC-5（実 FS/実 GUI/実
provider/cross-consumer gate）は外部 gate が所有するため Issue は
open 継続。

## 検証結果（本 slice 実走分）

| Command | OS | Toolchain | Exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows | rustc（mise） | 0 | Pass（`cargo fmt --all -- --check`） |
| `mise run check synvelis-library` | Windows | rustc（mise） | 0 | Pass（`cargo check --locked --all-targets -p synvelis-library`） |
| `mise run lint synvelis-library` | Windows | rustc（mise） | 0 | Pass（clippy lane、finding なし） |
| `mise run test synvelis-library` | Windows | rustc（mise） | 0 | Pass — unit 27・`tests/import.rs` 25・`tests/import_recovery.rs` 6・他 package suite すべて Pass、doc-test 失敗なし |
| `mise run build synvelis-library` | Windows | rustc（mise） | 0 | Pass（`cargo build --locked -p synvelis-library`） |
| `mise run test-rt` | Windows | rustc（mise） | 0 | Pass（`cargo test --workspace --locked rt_`、workspace 全 `rt_` filter test Pass） |
| `mise run test-msrv` | — | — | — | NotRun（POLICIES §5 により Daily Full CI 委譲） |

補助証拠（canonical = mise、cargo 直接実行は補助）:
`cargo test -p synvelis-library --test import` →
`running 25 tests` / `25 passed; 0 failed`。

### doc 監査

- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index apply --dir <dir>` — 触れた dir（`docs/designs`・
  `docs/plans`・`docs/tasks`・`docs/specs`・`docs/test-specs`）の
  index を生成経路で再生成。`docs/impl/ir` は
  `sync_impl_index.js --kind ir --apply`。結果は commit 時点の
  diff と `doc_status.js lint` rescan で確認（IR Validation に記録）。
- `node .agents/skills/doc-status/scripts/doc_status.js lint` /
  `audit`・`node .agents/skills/impl-doc/scripts/
  audit_impl_record.js` — 変更 doc の整合確認。新規 finding の有無は
  IR Validation と本ファイル更新時に記録する。repo-wide baseline
  （legacy `invalid-id-*` 系）は既知の migration 対象として区別。

## 外部 gate 残件（NotRun/tracking — 本 slice の検証対象外）

| Gate | Owner | Status |
| --- | --- | --- |
| 実 OS/FS durability・DirSync OS 差・Linux/macOS 実測 | synvelis#91 | NotRun |
| 統合試験（app 層 E2E） | synvelis#97 | NotRun |
| 実 GPUI/GUI/実 audio device・複数 Window | synvelis#114 | NotRun |
| real provider・実ネットワーク・remote LFS | synvelis#331 | NotRun |
| cross-consumer・AudioLens baseline・f64 比較 | ruviform#294 | NotRun |

mock/double/fixture/headless/scripted の PASS は上記 gate の証拠に
昇格しない。

## Requirement × Result 直交記録（POLICIES.md）

| Requirement | environment_kind | Result |
| --- | --- | --- |
| integrity 拒否 4 経路の typed error 実測 | deterministic backend + scripted double（fault 分類） | Pass |
| DirSync `Unsupported` の記録・publish 成否との分離 | scripted double（receipt 分類注入） | Pass（分類のみ。実 durability = Unsupported/NotRun） |
| `synvelis-library` 非回帰 | deterministic backend | Pass |
| RT 非阻害（`test-rt`） | deterministic backend | Pass |
| 実 OS/FS DirSync/durability | 実 FS | NotRun（synvelis#91） |
| 実 GUI/provider/cross-consumer | 実環境 | NotRun（#114/#331/ruviform#294） |
| MSRV lane | rust 1.89 | NotRun（Daily Full CI 委譲） |
