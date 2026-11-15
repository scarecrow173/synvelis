---
id: "EXP-rcp07-cross-gate"
type: "experiment"
status: "completed"
title: "RCP-07 cross-gate fixture/ledger slice 設計試行 (synvelis#296)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/rcp-07-cross-gate.md"
  depends-on:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/test-specs/rcp-07-cross-gate.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcp-07-cross-gate.md"
    - "docs/impl/ir/rcp-07-cross-gate.md"
    - "evidence/sv296/acceptance-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcp-render-recipe-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/rcp-07-cross-gate.md"
    - "docs/impl/ir/rcp-07-cross-gate.md"
    - "evidence/sv296/acceptance-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "cross-gate fixture を synvelis-render/tests/rcp07_gate.rs に集約（依存方向が player/project/resolver 型を名指せる唯一の test 位置）"
      - "store→resolver 接合は lookup() の CandidateReport を RenderCandidate に直接転記（eligibility を fixture で再判定しない）"
      - "branch adopt の oracle を独立構成した muted live project の resolve_live key に変更（branch resolution 出力を期待値に使わない）"
      - "queued cancel は JobOrchestrator::submit（non-driving）経路で検証（handle は同期 drive）"
      - "GATE-08..10 は ledger 分類記録のみ（stub/#[ignore] 疑似配置を作らない）"
    rejected:
      - "synvelis-app/tests/rcp07_gate.rs 新設 — app は synvelis-render 型を名指せず、両端は rcp_04/rcp06 fixture が被覆済み"
      - "adopt 後 live key == candidate branch key の当初仮定 — RecipeBranchPin が hashed identity で live/branch key を設計上分離するため不成立"
      - "既存 rcp01..06 fixture helper の共有化 — test file 間 helper 共有機構なし・contract 独立性のため各 file 自前 helper 慣行"
---

# RCP-07 cross-gate fixture/ledger slice 設計試行 (synvelis#296)

## 問題設定

synvelis#296（RCP-GATE-01..12 cross-gate tracker）の
fixture/ledger slice にあたり、merge 済み RCP-01..05 基盤を
またぐ cross-gate fixture をどこに・どの oracle で・どこまで
書くかについて 5 つの設計上の不確実点があった。

- (a) cross-gate fixture の test 配置: `synvelis-render` と
  `synvelis-app` のどちらに置くか（crate 依存方向の制約）
- (b) store→resolver 接合部で eligibility を誰が判定するか
  （fixture が独自判定すると contract を二重実装する）
- (c) branch adopt の独立 oracle: branch resolution の出力を
  期待値に使うと oracle が循環する
- (d) queued cancel の検証経路: `JobOrchestrator::handle` が
  同期 drive するため cancel が間に合わない可能性
- (e) 実行しない lane（実 plugin/OS/GUI/RT/consumer）を
  fixture でどう扱うか（stub/`#[ignore]` は実行証拠の偽装）

## 試行と結果

### (a) fixture 配置 — 採用: `synvelis-render/tests/rcp07_gate.rs` に集約

crate 依存方向（`synvelis-render` → `synvelis-player`/
`synvelis-project`/`synvelis-recording`）により、recipe・
artifact・orchestrator・`synvelis_player::rcp` resolver・
variant/branch 型を**一つの test file から名指せる**のは
`synvelis-render` test のみ。cross-gate の本質はこれら権威層の
**接合**であり、接合部を単一 crate の test に置くことで
「誰がどの contract を消費するか」を型で固定できる。
`synvelis-app` は `synvelis-render` に依存しないため render
型を名指せず、app test を置いても store→resolver 接合は
書けない（rcp06_render.rs 冒頭 doc comment と同じ制約）。
resolver→projection の両端は RCP-04（`rcp_04.rs`）と
RCP-06（`rcp06_render*.rs`）が既に pin 済みのため、app test
新設は coverage/cost 不均衡と判断し却下。

### (b) store→resolver 接合 — 採用: lookup verdict の直接転記

`lookup()` は `CandidateReport { artifact, eligibility,
reproducibility, access }` を返し、これは RCP-04 語彙
（`ArtifactEligibility`/`ReproducibilityClass`/`AccessState`）
そのもの。fixture はこの report を `RenderCandidate` に
転記するだけで、eligibility を独自判定しない — 「store が
verified と言ったものを resolver がどう扱うか」という
**権威間の受け渡し**だけを pin し、判定ロジックの二重実装を
避ける。quarantine 後は再 lookup の verdict で candidate を
再構成し、resolver が non-Exact に fail-closed することを
確認（GATE-06）。

### (c) branch adopt の oracle — 採用: 独立 project の再解決

`RecipeBranchPin` は hashed recipe 内の唯一の branch data
（`recipe.rs` doc 明記）で、pin 有無で live/branch key は
**設計上分離**される。当初「adopt 後 live key == candidate
branch key」を仮定したが実測で不成立と判明。oracle を
「同じ acoustic edit（clip mute）を施した**独立 live
project** の `resolve_live` key」に変更 — adopt が candidate
acoustics を live に具現化したことを、branch resolution の
出力を期待値に使わず検証できる。`k_cand != k_live_after`
の分離自体も GATE-05 の pin 対象として assert。

### (d) queued cancel の検証経路 — 採用: `submit`（non-driving）

`JobOrchestrator::handle` は non-coalesced submit を即
`drive` する（orchestrator.rs:484-507）ため、queued cancel
を `handle` 経由で検証すると job が既に terminal で
`cancel()` が false を返す（初回実行で実際に失敗を確認）。
queued cancel は `submit`（non-driving 入口）+ `cancel` +
state/artifacts 検証で pin した（GATE-04）。

### (e) 実行しない lane — 採用: ledger 分類記録のみ

実 Window/GPUI runner・実 plugin ABI・実 PCM clock・
AudioLens consumer は全て外部 gate。実行しない lane を
`#[ignore]` test や stub で配置すると「実行していない証拠を
fixture で偽装」するため、ledger の status 語彙
（NotRun/Deferred/Blocked/Unsupported）と所有 gate 明記に
留めた。判定規則（実測なし Pass=0・headless≠実機）も
ledger に明文化。

## Expected vs Observed（実測）

| 実験 | Expected | Observed |
| --- | --- | --- |
| `cargo test --test rcp07_gate` 初回 | 7/7 green | 5/7 — gate04 cancel false（handle 同期 drive が原因）・gate05 key 不一致（branch pin hashed が原因） |
| 修正後 | 7/7 green | 7/7 green（exit 0） |
| `adopt 後 live key == candidate key`（仮説） | equal | **不成立** — pin hashed で設計上分離。oracle を独立 muted live project に変更して pin |
| `mise run lint synvelis-render` | clean | 初回 5 error（assert_is_empty×3・used_underscore_binding・match_wildcard）→ 修正後 clean |

## 結論

cross-gate fixture は `synvelis-render` test に集約し、
eligibility の権威を store lookup verdict に一元化、
branch adopt は独立 project oracle で検証、queued cancel は
`submit` 経路で pin、実行しない lane は ledger 分類のみ —
の 5 判断で確定。`synvelis-app` test は追加しない（依存
境界と既存 fixture 被覆から proportionate）。GATE-05 の
RCP-05 未整備部は #294 完了待ちとして Blocked 記録。
