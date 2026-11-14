---
id: "TSPEC-g7v0galTaViQdO2RpCak1D"
type: "test-spec"
status: "approved"
title: "SF-06 cross-gate テスト仕様 — headless fixture 被覆棚卸し・PCM oracle (synvelis#109)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf06-cross-gate.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sf-06-cross-gate-oracle.md"
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/impl/ir/sf-06-cross-gate-headless-oracle.md"
    - "evidence/reports/sf-06-cross-gate-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf06-cross-gate.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-06 cross-gate テスト仕様 — headless fixture 被覆棚卸し・PCM oracle

## Purpose

Issue #109 の headless slice が要求する「既存 SF-01〜SF-05
headless fixture の被覆棚卸し」「欠落 fixture の追加」
「evidence 台帳での lane 分離」を fixture で固定する。実 OS
GUI・device・plugin・consumer・runtime engine の lane は対象外
（**NotRun**）。

先行 slice（`sf-06-cross-gate-oracle`）が project 層の独立
oracle・fault・migration を固定したのに対し、本 slice は
**player/render 層の PCM 実測が欠落していた send/bus 経路**を
対象にする。

## Feature

### Feature: send/bus 経路の PCM 実測 oracle

`RoutingModel`（persisted 公開フィールド）から導出した解析的
期待値と、`RoutingEngine` が render した `PlayReport.pcm` を
frame/sample 単位で比較する。lowering/source-map 層の検証
（`sf03_l1` の `assert_wires_send_tap_into_processor` 等）が
「配線が存在する」ことを示すのに対し、本 fixture は「その配線が
正しい信号を運ぶ」ことを PCM で証明する。

## Rules

- Rule: fixture は `synvelis-player` の公開 API
  （`ProjectController`・`RoutingEngine`・`prepare`・
  `import_local`・`fixture` helper）のみを使い、`src/` 内部・
  private helper を参照しない。
- Rule: PCM 期待値は fixture 関数 `fixture_sample_f32` と
  解析的 gain だけから導出する — 実装の出力を期待値に使わない
  （独立 oracle 原則は先行 slice と同じ）。
- Rule: mute/solo の検証は「測定 0.0」を assert する。
  `NotLowered`/`Unavailable` の状態語彙と PCM silence を混同
  しない（`evidence/sv109/coverage.md` の対応表を参照）。
- Rule: routing edit の atomicity は「1 `RoutingEditIntent` =
  1 `ProjectRevision` = 1 undo step」を revision counter と
  PCM 復元の両方で検証する。
- Rule: headless fixture の Pass は GUI-L2・EXT-L3・実 plugin・
  実 device・consumer 受入へ昇格しない。#114（GPUI Editor）向け
  の case 対応表は `evidence/sv109/coverage.md` に記録する。

## Examples

### Example: pre/post send tap の識別

- Given track clip → strip（fader 0.5）→ master に加え、
  `PreFader` send gain 0.5 と `PostFader` send gain 0.25 が
  同一 return bus → master へ繋がる project、
- when `RoutingEngine` を prepare→publish→run で render、
- then master PCM は全 frame で `1.125 · fixture_sample_f32`
  （clip window 内）/ `0.0`（window 外）に一致する。
  pre tap が誤って post-fader 信号を運ぶ場合の総和は
  `0.875 · x` となり本 assert は失敗する — tap 点を識別する。

### Example: graph edit の PCM 不変条件

- Given clip → master 直結（fader 1.0）の project で PCM = x、
- when bus+send+return を 1 intent で追加（revision +1）、
- then render PCM = `1.75 · x`（x + 0.5x + 0.25x）。save→load
  後の再 render は同一 PCM/CRC。undo 1 回で pre-edit PCM に
  bit-for-bit 復帰する。

### Example: mute/solo の send 経路への波及

- Given 2 track（disjoint clip window）各々 PostFader send 0.5
  → shared return bus → master、全 fader 1.0、
- when A を mute → A window は測定 0.0・B window は `1.5 · x_B`、
  A を unmute+solo → A window のみ `1.5 · x_A`・B window は
  測定 0.0、
- then mute/solo が main edge と send edge の双方を PCM レベル
  で制御することを確認する。

## #114 (GPUI Signal Flow Editor) Case 対応表

headless fixture が被覆する意味論と、#114 が GUI で再検証すべき
case の対応。headless Pass は #114 の Pass を構成しない。

| headless case | #114 での対応検証 |
| --- | --- |
| `sf06_send_pcm_pre_vs_post_tap` | Editor 上の pre/post send 表示が実 tap と一致するか（表示≠実測の確認） |
| `sf06_graph_edit_pcm_undo_save_reload` | Editor gesture → undo/save/reload が同じ PCM を維持するか |
| `sf06_mute_solo_pcm_through_sends` | Editor の mute/solo 操作が send 経路へ波及するかの GUI 確認 |
| `evidence/sv109/coverage.md` の SF-PROBE 行 | probe 状態表示（Unavailable/Unobserved/Unknown）と実観測の一致 |

## NotRun / Deferred

- 実 OS GUI・native dock・実 plugin・実 device・AudioLens
  consumer: `evidence/sv109/external-gates.md` 参照。
- SF06-RCP-01・EX-001〜020・Gate A/B/C・Windows L2/L3・
  Ruviform #219/#294 提出分: 対応表に NotRun/Deferred として
  記録（`evidence/sv109/coverage.md` 末尾）。
