---
id: "IR-SV84-syn-io-02"
type: "ir"
status: "final"
title: "SYN-IO-02 store.rs StagedFile 移行 実装記録・検証証跡"
created: "2026-11-07"
owners: ["scarecrow173"]
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
