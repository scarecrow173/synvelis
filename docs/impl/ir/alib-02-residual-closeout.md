---
id: "IMPL-034VMQZEdjKG5qiL5uYYuF"
type: "impl"
status: "completed"
title: "ALIB-02 residual closeout — Issue #86 DoD 照合・canonical lane 実測・残存 gate 棚卸し"
created: "2026-11-11"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "docs/specs/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/designs/alib-02-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-residual-closeout.md"
      - type: "file"
        path: "evidence/issue-86-v5-migration/closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/specs/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/plans/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/test-specs/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/test-specs/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/alib-02-residual-closeout.md"
    - "docs/plans/alib-02-residual-closeout.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
    - "docs/impl/ir/issue-86-verified-import-wiring-slice.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-02-residual-closeout.md"
    - "docs/impl/ir/alib-02-streaming-verified-import.md"
    - "docs/impl/ir/alib-02-v5-snapshot-compat.md"
    - "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/alib-02-residual-closeout.md"
  derived-by: []
  verifies:
    - "docs/test-specs/alib-02-streaming-verified-import.md"
    - "docs/test-specs/alib-02-v5-snapshot-compat.md"
    - "docs/test-specs/issue-86-verified-import-wiring.md"
  verified-by: []
  references:
    - "evidence/issue-86-v5-migration/README.md"
    - "evidence/issue-86-v5-migration/closeout-verification.md"
    - "evidence/issue-86-verified-import-wiring/verification-ledger.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ALIB-02 residual closeout — Issue #86 DoD 照合・canonical lane 実測・残存 gate 棚卸し

## Summary

Issue #86（ALIB-02）の簿記クローズアウト。プロダクションコードの
変更は一切なく、受入条件 5 行・残実装 DoD 4 行を実在する
fixture・テスト名・canonical lane 実測結果と 1:1 で照合し、
doc chain・evidence・本 IR に固定した。外部 gate（実 OS/FS・
実 GUI・AudioLens/cross-consumer・real provider/remote LFS・
統合試験）は `NotRun` として所有 Issue を記名し、headless PASS
を実機証拠に昇格しない方針を維持した。

照合の結論: headless 範囲の DoD(1)〜(4) と受入条件 AC-1〜AC-4 は
実測済み証拠で充足。AC-5（実 OS/FS 差・RT 非阻害・AudioLens
baseline・Ruviform IO-01 との DAG/責務差の実機証拠）は外部 gate
が残り、Issue は open 継続が前提。

## Context

- Issue: <https://github.com/scarecrow173/synvelis/issues/86>
  （ALIB-02 本体。2026-11-10 の close 判定で残実装 DoD 4 行が
  切り出された経緯あり）
- 統合済み実装 PR: #220（library slice・streaming verified
  import）・#236（ALIB-07 fault matrix）・#253（v5
  digest/migration slice）・#256（ALIB-07 import cancel/crash
  slice）・#326（DoD(2) consumer 配線）・#328（DoD(1) 実 v5
  fixture）。
- 本 slice の上流 doc chain:
  `docs/specs|designs|plans|tasks/alib-02-residual-closeout*`。
- 証拠: `evidence/issue-86-v5-migration/`（本 slice で
  `closeout-verification.md` を追記）・
  `evidence/issue-86-verified-import-wiring/verification-ledger.md`。

## Implementation

### Changes（docs/evidence のみ。src 変更なし）

- 新規 doc chain 5 件:
  `docs/specs/alib-02-residual-closeout.md`（status `implemented`、
  受入条件/DoD 照合 ledger）・
  `docs/designs/alib-02-residual-closeout-design.md`（status
  `approved`）・`docs/plans/alib-02-residual-closeout.md`（status
  `completed`）・`docs/tasks/alib-02-residual-closeout.md`（status
  `done`）・本 IR。
- `evidence/issue-86-v5-migration/closeout-verification.md`:
  canonical lane 7 本の実走結果・再現手順・照合表の要約。
- 既存 doc の整合（編集許可範囲内）:
  - `docs/specs/alib-02-v5-snapshot-compat.md`: status
    `approved` → `implemented`（PR #253/#328 で実装統合済みの
    実態に追従）。`derives-from` に Issue #86 URL を追加して
    upstream traceability を充足。`deferred-by` に streaming
    design を追加して reciprocal relation を整合。
  - `docs/plans/alib-02-v5-snapshot-compat.md`: `implemented-by`
    に task と 2 件の IR を充填。
  - `docs/test-specs/alib-02-streaming-verified-import.md`:
    `import.rs` のテスト数記述を実測 15 → 21 に訂正（PR #236・
    #256 の fault/cancel 追加分を反映）。
  - `docs/test-specs/alib-02-v5-snapshot-compat.md`:
    `verified-by` に task を追加して reciprocal 整合。
  - `docs/impl/ir/alib-02-streaming-verified-import.md`:
    Follow-ups を「v5 migration・consumer 配線は headless で統合
    済み、外部 gate は #114/#97 が所有」の実態に更新。
  - `docs/impl/ir/alib-02-v5-snapshot-compat.md`: related 追記・
    残件注記を closeout 実態に整合。
  - `docs/impl/ir/alib-02-v5-snapshot-compat-slice.md`:
    Verification を本 slice の再実測参照に更新。

### Issue #86 受入条件/DoD 照合結果

詳細な対応表は `docs/specs/alib-02-residual-closeout.md` の
Acceptance Criteria 節と本 IR の Validation を参照。要約:

- AC-1（bounded memory・WAV Golden/ID/PCM/Save/Reload/Undo/Redo/
  Record/Playback/E2E 維持）: headless 実測済み
  （`bounded_memory_streaming_with_tiny_chunks`、
  `import_recovery.rs` 6 tests、`verified_import.rs` 8 tests、
  workspace `mise run test` Pass）。app/GUI 層の Record/Playback/
  E2E は synvelis#114・#97 の外部 gate として NotRun 分離。
- AC-2（ImportId replay・重複/fault/Crash・StaleRevision・対象
  消失・他 Track 誤配置防止）: `import.rs` 21 tests +
  `import_recovery.rs` + `verified_import.rs` で実測。複数
  Window 誤配置は GUI 領域のため synvelis#114 gate。
- AC-3（SHA-256/旧 blob namespace 非衝突・v5 lossless/Degraded
  読み込み・Provenance/ID/opaque 維持）:
  `alib02_digest.rs` 5 tests・`alib02_v5_real.rs` 8 tests・
  namespace 衝突/共存/CRC collision 各テストで実測。
- AC-4（Staging→Verify→Published→LiveCommit→Save/HEAD 各障害点
  の注入・Indeterminate/DirSync の正直な分類・Orphan 保護/明示
  GC/復旧）: fault matrix 系テスト群で実測。
- AC-5（Linux/Windows/macOS FS 差・RT 非阻害・AudioLens
  baseline・Ruviform IO-01 との DAG/責務差の証拠記録）:
  **未充足（外部 gate）**。Windows 実測と `test-rt` lane・責務差の
  文書記録は実施済み。Linux/macOS 実 FS は synvelis#91、
  AudioLens/cross-consumer は ruviform#294、実 provider/remote
  LFS は synvelis#331、実 GUI/統合試験は synvelis#114・#97 の
  gate として NotRun。
- 残実装 DoD(1)〜(4): すべて headless 実測で充足
  （PR #253・#256・#326・#328 の統合済み実装 + 本 slice の
  再実測）。

### 判断と根拠

- **status `implemented` 化の根拠**: `alib-02-v5-snapshot-compat`
  spec は PR #253（digest 分類・v10→v11 step）と PR #328（実 v5
  fixture）で実装が main に統合済みであり、`approved` のまま
  残すのは stale-upstream drift だった。実装・検証・evidence が
  揃った時点で `implemented` が実態。
- **テスト数訂正の根拠**: `docs/test-specs/
  alib-02-streaming-verified-import.md` は `import.rs` を 15
  tests と記述していたが、PR #236（ALIB-07 fault matrix）と
  #256（cancel/crash slice）で fault/cancel 系 6 tests が追加
  され、実測は 21 tests。doc を実測に追従させた。
- **index README 非編集の判断**: `docs/*/README.md` は
  Coordinator が merge 時に再生成する運用のため、本 slice では
  触らない。新規 doc の `index-missing-entry` finding は merge
  時の再生成で解消される既知の一時状態として扱う。
- **境界外 drift の扱い**: `docs/specs/
  issue-86-verified-import-wiring.md` の status `approved`、
  `docs/specs/alib-02-streaming-verified-import.md` の未チェック
  acceptance 行など、編集許可リスト外の残存簿記は本 slice では
  変更せず、Follow-ups と PR 本文に記録して Coordinator の
  merge 時判断に委ねる。

## Related Experiments

### Adopted

なし（本 slice は実験を伴わない簿記作業）。

### Rejected

なし。

## Validation

canonical lane 7 本を `mise run <task>` 経由で worktree
（Windows/NTFS、toolchain Rust 1.99.0、対象 commit は本 PR の
head）で実走した。結果の詳細・再現手順は
`evidence/issue-86-v5-migration/closeout-verification.md` を参照。

| Lane | Command | Result |
| --- | --- | --- |
| fmt | `mise run fmt-check` | Pass (exit 0) |
| check | `mise run check` | Pass (exit 0) |
| lint | `mise run lint` | Pass (exit 0, `-D warnings`) |
| test | `mise run test` | Pass (exit 0 — 2,091 tests / 202 binaries, ALIB-02 5 binaries 48 tests all ok) |
| build | `mise run build` | Pass (exit 0) |
| rt | `mise run test-rt` | Pass (exit 0 — 104 `rt_` tests) |
| msrv | `mise run test-msrv` | Pass (exit 0 — rust 1.89.0、2,091 tests / 0 failed) |

doc 監査（2026-11-12 実走）:

- `doc_status.js lint --type all`（repo 全体、758 docs / 647
  findings — 大半は legacy invalid-id-format 等の既存 baseline
  migration 対象）: 本 slice の新規 finding は新規 doc 4 件
  （spec/design/plan/task — impl は lint 対象外）の
  `index-missing-entry`/`orphan-*`（`docs/*/README.md` は merge 時に
  Coordinator が再生成する既知の一時状態）と、spec/IR の
  `references` が指す `closeout-verification.md` 未作成時の
  broken-relation-link のみで、同ファイル作成により解消。
  `docs/specs/README.md` の `index-metadata-mismatch`
  （`alib-02-v5-snapshot-compat` approved→implemented）も同再生成で
  解消。`traceability-missing-upstream`（streaming spec）は編集境界外の
  ため残存として記録。
- `doc_maintenance.js plan --type all`: deterministic repair は
  index rebuild（designs/plans/specs/tasks/test-specs README）のみ。
  apply は merge 時の index 再生成に委ね未実施。
- `audit_impl_record.js --dir docs/impl/ir`: 本 IR に error なし。
  `alib-02-v5-snapshot-compat-slice.md`（missing-section）・
  `alib-02-v5-snapshot-compat.md`（metadata）・
  `issue-86-verified-import-wiring-slice.md`（missing-section）の
  error は HEAD 時点の既存 baseline であり本 slice では増やしていない。

照合に使った実測コマンド（抜粋）:

- `rg -n "^fn |^    fn |#\[test\]" crates/synvelis-project/tests/alib02_*.rs crates/synvelis-library/tests/import*.rs crates/synvelis-player/tests/verified_import.rs` — 実在テスト名の列挙。
- `ls crates/synvelis-project/tests/fixtures/` — 実 v5 fixture
  （`alib02-v5-legacy.json`・`.blob`）の存在確認。

## Risks

- **doc drift の再発**: 編集境界外の doc（wiring spec の status、
  streaming spec の acceptance checkbox、legacy invalid-id 群）
  に残存する drift は本 PR では解消できない。後続の bookkeeping
  PR または Coordinator の merge 時判断が必要。
- **lane の環境依存 flake**: `mise run test` 系は過去に webdav
  flake・LNK1102 の観測例がある。Fail が出た場合は単独再実行で
  切り分け、最終結果を正直に記録する方針。
- **index-missing-entry の一時状態**: 新規 doc 4 件（spec/design/
  plan/task）は merge 時の index 再生成まで `index-missing-entry`
  finding を持つ。これは仕様上の既知状態であり、blocking として
  残るが merge 手順で解消される。

## Follow-ups

- Issue #86 の close 判定: AC-5 の外部 gate（synvelis#91 実
  OS/FS・#97 統合試験・#114 実 GPUI/GUI・#331 real
  provider/remote LFS・ruviform#294 cross-consumer/AudioLens）が
  完了するまで Issue は open 継続。
- 編集境界外の残存簿記（`issue-86-verified-import-wiring` spec
  の status、`alib-02-streaming-verified-import` spec の
  acceptance checkbox、legacy invalid-id-format 群）は後続の
  bookkeeping PR で対応。
- `docs/*/README.md` index の再生成は Coordinator が merge 時に
  実施。
