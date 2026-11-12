---
id: "IR-SV84-syn-io-02"
type: "impl"
status: "completed"
title: "SYN-IO-02 store.rs StagedFile 移行 実装記録・検証証跡"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SYN-IO-02 store.rs StagedFile 移行 — 実装記録・検証証跡

## 対象

- Issue: https://github.com/scarecrow173/synvelis/issues/84
- PR: https://github.com/scarecrow173/synvelis/pull/178
- Branch: `feature/project-store-staged-file`
- Head SHA: `dfc06b5df8765e800d80ffe2bd70b03b849ed76e`
- Ruviform 解決 SHA: `49de02cc530d324772d8556225030eb2b9307751`
  （`Cargo.lock` 内の全 ruviform crate と一致・lock 差分は新規
  `ruviform-io-file` 1 package + `synvelis-project` dep edge のみ）

## P1 修復（2026-11-07・merge 前レビュー Judge=BLOCKED 由来）

- **根因**: `save` の `DestinationExists` 受理経路で
  `SnapshotCollision::Satisfied => {}` が「内容冪等性」を
  「耐久性達成」と同一視していた。初回 publish が
  `Published`（rename 済み・FileSynced）→ DirectorySync 失敗で
  `Err` を返すと、`snapshots/r<N>.json` は存在するが directory
  entry 耐久性が未証明のまま残る。同一 byte 再送は CreateNew
  が `DestinationExists` で即 `Err`（publish phase・親 dirsync
  未実行）→ byte 一致で Satisfied → HEAD `Replace` + root
  dirsync だけ成功して `Ok` を報告 — `snapshots/` 親 dir は
  HEAD 親 root とは別であり、どこでも dir sync されないため
  電源断/復旧で参照耐久性が崩れていた。
- **修復（受理/最終公開境界）**: 衝突受理をスキップから
  「勝者 byte の完全 publish 再実行」へ修正。`Satisfied` は
  stored byte（byte 同一・既存新 watermark 保持の双方を含む）、
  `Refresh` は incoming byte を `Replace` +
  `FileAndDirectorySync` で再公開 — `PublishReceipt` の Ok は
  要求した全保証達成時のみなので、受理が dir sync 証拠を必ず
  再確立する。公開 API には standalone dirsync が無いため、
  完遂 publish が唯一の再確立経路。`Conflict` 拒否・HEAD
  ordering・canonical byte・CAS/外部変更・watermark 単調増・
  InMemory port 整合は不変（InMemory は byte 勝者意味論のみ、
  FS 耐久性の次元を持たない）。
- **決定的 fault 試験（tests/store_publish.rs・Rule 8 対応）**:
  - `satisfied_resave_reestablishes_snapshot_durability`:
    HEAD 未指向の post-fault 状態 + 過去 mtime sentinel で
    同一 byte `save` → destination が rename で物理更新
    （sentinel 消滅＝publish pipeline 再実行の証拠）かつ byte
    一致・HEAD repoint・load 成功。修復前は sentinel 保持で
    RED → 修復後 GREEN。
  - `satisfied_resave_fails_instead_of_reporting_false_success`:
    Windows は destination を `FILE_SHARE_READ` のみで保持
    （collision read は通るが atomic replace 不可）、unix は
    `snapshots/` read-only 相当で再公開を完遂不能に → `Err`・
    HEAD 非 repoint。修復前は `Ok("r1.json")` の偽成功で RED →
    修復後 GREEN。実電源断は未実施（mechanism pin のみ）。
- **レビュー状態**: Reviewer B + 独立 Validator が P1 CONFIRMED・
  Judge BLOCKED（merge 保留）→ 修復後の独立 Validator 再検証は
  **CONFIRMED-FIXED**（勝者 byte のみ Replace・HEAD ordering・
  Conflict/CAS/InMemory parity を全点検、新規違反なし）。
  残余（non-blocking）: ≥3 同時 writer での read→Replace 間
  TOCTOU（single-writer モデル非目標・CAS rename プリミティブ
  不存在）と、unix 側 fault leg が修復前判別しない点
  （test comment 記載どおり RED 判別は Windows leg）。
- **修復後検証（Windows・実測）**: `mise run fmt`/`fmt-check`/
  `check`/`lint` 全て pass。`mise run test`（workspace --locked）
  全 83 test binary 0 fail — `store_publish` 16 tests 含む
  （修復前 RED 再現済み: 2 new tests が現行コードで失敗、
  false success `Ok("r1.json")` を観測）。未実施: 実電源断・
  macOS/Linux 実測（Refs #84 受入 Gate 維持）。

## 実施内容

- `DirectoryBundleStore::write_atomic`（`.tmp` → `File::sync_all`
  → `fs::rename`）を撤去し、`publish_staged` helper として
  `StagedFile::create` → `Write` → `seal()` → `publish` へ移管。
- snapshot = `PublishMode::CreateNew`、HEAD = `PublishMode::Replace`
  （snapshot publish 成功後にのみ）。両者
  `Durability::FileAndDirectorySync`、`RequireAtomic`。
- `PublishFailure` → `Error::Store` 写像は phase/kind/effect/
  durability/operation id（`PublishFailure` Display 全文）+ temp
  path + root io cause（`Error::source`）を転写。`Indeterminate`
  残滓は自動削除せず、`NotPublished`/`Published` は best-effort
  cleanup。成功 publish の retained temp も best-effort cleanup。
- 同一 revision 衝突は `snapshot_collision` で分類:
  byte 完全一致 → `Satisfied`（冪等、HEAD 再指向のみ）、envelope
  が `id_watermark` のみ異なる同一論理 snapshot → 単調増方向
  `Refresh`（`Replace` 再公開）/ 低位方向 `Satisfied`、それ以外
  → `Conflict`（既存 byte・HEAD 不変、`Error::Store`）。
- `InMemoryStore::save` を同一分類器に整合。
- 依存追加: `ruviform-io-file` git dep（root `Cargo.toml` +
  `crates/synvelis-project/Cargo.toml`）+ `Cargo.lock` 再生成。
- Doc chain: spec/design/plan/test-spec/task を新規作成・同期済み。

## 変更ファイル

- `crates/synvelis-project/src/store.rs`
- `crates/synvelis-project/tests/store_publish.rs`（新規・14 tests）
- `Cargo.toml`、`crates/synvelis-project/Cargo.toml`、`Cargo.lock`
- `docs/{specs,designs,plans,test-specs,tasks}/syn-io-02-*` 5 件 +
  各 `README.md` index

## 検証証跡（実行環境 Windows・全 pass）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt` | pass |
| `mise run fmt-check` | pass |
| `mise run check`（cargo check --workspace --all-targets） | pass |
| `mise run lint`（cargo clippy --workspace --all-targets -- -D warnings） | pass |
| `mise run test`（cargo test --workspace --locked・CARGO_BUILD_JOBS=2） | pass・83 test binary・0 fail |

契約テスト（`tests/store_publish.rs`・test-spec E1–E11 対応）:
publish 順序/両 snapshot 残存、異 byte 同名拒否・byte 同一冪等、
watermark 単調増 refresh・低位 Satisfied、孤児 snapshot の
Satisfied 再指向復旧、外部占有 destination 拒否・HEAD 不変、
HEAD publish 失敗（NotPublished）で orphan+旧 HEAD、外部 HEAD
書換えの正直報告、`*.rvf-tmp-*` 残滓非干渉、再 Open 再現、
空 bundle head()=None、byte-faithful round-trip、stage create
失敗、InMemoryStore port 整合、同名 CreateNew 並行 single-winner。

## 独立レビュー（reviewer A/B → validator → judge・全て独立 subagent）

- Reviewer A（correctness）: IC-1 stage ラベル、IC-2/IC-3 docs 矛盾、
  IC-4 cleanup guard、IC-5 TOCTOU residual — 全て対処済み/記録。
- Reviewer B（adversarial）: IC-B1 `id_watermark` 同一 revision 異 byte
  save 正当経路（P1 → `snapshot_collision` Refresh/Satisfied で修正）、
  IC-B2 HEAD 前進過剰主張（docs/rustdoc 修正）、IC-B3 root cause 欠落
  （`source()` 転写で修正）、IC-B4 非空 bundle `project.new` 拒否
  （意図的強化として記録）、IC-B6 テスト gap（Stage 失敗・Refresh・
  orphan repoint・InMemory refresh を追加）。
- Validator: 7 claim 全て CONFIRMED + 新規候補（`fs::read` 失敗の
  分類落ち → read error 表面化で修正、残りは residual 記録）。
- Judge: **ACCEPT WITH NON-BLOCKING NOTES**・blocking issue なし。

## Non-blocking notes（PR 記載済み）

- `project.new` 等の非空 bundle への silent clobber が明示拒否へ
  変わった（意図的強化・behavior change）。
- classify→Replace の TOCTOU は単一 writer モデルの residual。
- `Satisfied` は HEAD を無条件再指向（孤児 snapshot 復旧の意図）。
- `Refresh`/`Satisfied` は stored byte が envelope として parse
  できる前提（load が検証境界）。

## NotRun / Unsupported

- OS 固有 Indeterminate/DirSync 失敗注入: ruviform-io-file 側
  fault-injection 責務へ委譲（consumer 側は失敗分類・HEAD 非前進
  経路を pin）。
- macOS/Linux 実測: NotRun（OS 中立 API のみ・Windows 実測済み）。
- 複数 process の真 HEAD CAS: 単一 file replace で実現不能
  （設計 deferred・単一 writer モデル外）。
- WAV Export(#25)・他 crate tmp/rename・SV112 UI・SV85 Provider・
  SV86 Asset Schema: 所有範囲外・未変更。
- RF #305 旧 Media API 撤去との結合: coordinator 統合判断へ連携、
  本 PR 単体では両 Consumer 受入を主張しない。
