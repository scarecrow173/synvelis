---
id: "IMPL-It7Fs3X3THd8sySW8lROw9"
type: "impl"
status: "completed"
title: "SF-06 cross-gate headless fixture 被覆棚卸し・欠落 fixture 追加 実装記録 (synvelis#109)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/sf06_send_path_pcm.rs"
      - type: "file"
        path: "evidence/sv109/coverage.md"
      - type: "file"
        path: "evidence/sv109/verification.md"
      - type: "file"
        path: "evidence/sv109/external-gates.md"
      - type: "file"
        path: "docs/specs/sf06-cross-gate.md"
      - type: "file"
        path: "docs/test-specs/sf06-cross-gate.md"
      - type: "file"
        path: "docs/impl/ir/sf06-cross-gate.md"
      - type: "file"
        path: "docs/impl/exp/sf06-cross-gate.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/specs/sf06-cross-gate.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/test-specs/sf-06-cross-gate-oracle.md"
    - "docs/impl/ir/sf-06-cross-gate-headless-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sf06-cross-gate.md"
    - "docs/impl/exp/sf06-cross-gate.md"
    - "evidence/sv109/coverage.md"
    - "evidence/reports/sf-06-cross-gate-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/sf06-cross-gate.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-06 cross-gate headless fixture 被覆棚卸し 実装記録

## Summary

Issue #109 の headless fixture slice。SF-01〜SF-05 の
headless テスト被覆を SF-MODEL/VALID/ACT/PROBE case 行列へ
棚卸しし、PCM 実測が欠落していた send/bus 経路へ fixture を
追加し、`evidence/sv109/` に台帳を記録した。

## Context

`docs/specs/sf06-cross-gate.md`・`docs/test-specs/
sf06-cross-gate.md` に基づく。先行 slice
（`sf-06-cross-gate-oracle`）が project 層の独立 oracle・
fault・migration を固定済み。本 slice はその T2 lane
（player/render の PCM 実測）の被覆棚卸しと欠落分の追加。

## Implementation

### Changes

#### `crates/synvelis-player/tests/sf06_send_path_pcm.rs`（新規）

3 テスト。全て `synvelis-player`/`synvelis-project` の公開
API のみを使用し、`src/` 内部・private helper を参照しない。

- `sf06_send_pcm_pre_vs_post_tap` — fader 0.5 で pre/post
  send tap を識別。master PCM = `1.125·x`（誤 tap なら
  `0.875·x` で fail）。bus→master `Main` edge の集約を含む。
- `sf06_graph_edit_pcm_undo_save_reload` — `RoutingEditIntent`
  で bus+send+return を 1 revision/1 undo で commit。
  save→load 往復で PCM/CRC bit 一致、undo で pre-edit PCM
  に bit 復帰。
- `sf06_mute_solo_pcm_through_sends` — 2 track disjoint clip
  window + shared return bus で、mute が main+send 双方を
  測定 0 に、solo が他 strip の main+send を除去することを
  PCM で確認。

#### `evidence/sv109/`（新規 3 ファイル）

- `coverage.md` — SF-MODEL-01..07・SF-VALID-01..08・
  SF-ACT-01..09・SF-PROBE-01..09 の被覆写像。先行 ledger
  （`evidence/reports/sf-06-cross-gate-ledger.md`）が
  SF-03/04 merge 前の stale な T2 `NotRun` を抱えていたため、
  現行 tests/ 全件の実測 green に基づく被覆の正本として
  新規作成。#114 case 対応表・外部提出分対応表を含む。
- `verification.md` — 実行コマンド・OS/toolchain/SHA・
  Expected/Observed・exit code の記録。
- `external-gates.md` — GUI-L2/EXT-L3・実 plugin/device・
  consumer・Ruviform #308/#309・EX-001〜020/Gate A/B/C・
  SF06-RCP-01 の NotRun/Deferred 判定。

#### docs 4 点（新規）

- `docs/specs/sf06-cross-gate.md` — 本 slice の spec。
- `docs/test-specs/sf06-cross-gate.md` — test 仕様 + #114
  case 対応表。
- `docs/impl/ir/sf06-cross-gate.md` — 本記録。
- `docs/impl/exp/sf06-cross-gate.md` — 被覆監査の experiment
  log。

#### 変更しなかったもの

- `src/` 本番コード・`Cargo.toml`/`Cargo.lock`/`mise.toml`/
  `.github/workflows/` — 全て無変更。
- `docs/*/README.md`（index）— `doc-maintenance` で再生成のみ。
- 既存 test ファイル — 被覆済み case への重複追加なし。

### Decisions During Implementation

- PCM 期待値は `fixture_sample_f32` × dyadic gain のみから
  導出（実装出力を期待値に使わない独立 oracle 原則）。
  全 gain（1.0/0.5/0.25/1.125/1.75/1.5）が 2 進で正確なため
  `assert_eq` の bit 一致を要求できる。
- 各 render は fresh `RoutingEngine` で prepare→publish→run
  を独立に行い、revision 間で engine 状態を持ち越さない。
- `wire_sends` helper は `CreateBus`+`CreateConnection`+
  `CreateSend` を 1 `RoutingEditIntent` にまとめ、SF-02 の
  atomic commit 契約を PCM レベルで検証する。
- `RoutingEditIntent` は gesture-local `TempBusId`/
  `TempSendId`/`TempConnectionId` を使い、persisted ID と
  衝突しない。`SendGainSpec::Create` が新規 send に専用
  `SendLevel` target を seed する。

## Related Experiments

### Adopted

- `docs/impl/exp/sf06-cross-gate.md` — 被覆監査の進め方・
  fixture 設計の検討（model assert / source map assert /
  PCM oracle の 3 候補比較）・実装中の発見を記録。

### Rejected

- なし（被覆済み case への重複 fixture 追加は監査段階で
  排除済み — exp E1/E2 参照）。

## Validation

`evidence/sv109/verification.md` 参照。`mise run fmt-check`・
`check`/`lint`/`test synvelis-player`・`test synvelis-project`・
`test synvelis-render` が全て exit 0。

## Risks

- `check`/`lint` の `synvelis-project`/`synvelis-render` は
  scoped 検証の対象外（同 crate の src/tests 無変更）。
  変更 crate は `synvelis-player` の test のみ。
- headless fixture の Pass を GUI/device/plugin/consumer の
  Pass に昇格しない — `external-gates.md` で NotRun/Deferred
  を明示。

## Follow-ups

- merge 後対象 SHA の ci.yml covering run（NotRun pending —
  Coordinator が merge 後 CI 証拠を確認）。
- doc-driven-dev-graph router が repo 全体の既存 broken
  relation（本 slice 無関係の deferred 記述）で `blocked`
  を返す件 — canonical docs は task 指定の 4 path +
  evidence で記録した。graph の健全性は別途保守対象。
