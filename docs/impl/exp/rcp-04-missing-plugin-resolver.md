---
id: "EXP-rcp-04-cutset-reconciliation"
type: "experiment"
status: "completed"
title: "RCP-04 Missing Plugin Resolver cutset・reconciliation 設計試行 (synvelis#293)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/293"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
  depends-on:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
    - "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcp-render-recipe-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
    - "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "edge_ok による covered_sources 限定記録"
      - "optional min==0 入力の dangling 判定除外"
      - "LayoutKey による決定的 port layout 比較"
    rejected:
      - "unknown plugin state を preview で Blocked 扱いする初期期待"
---

# RCP-04 Missing Plugin Resolver cutset・reconciliation 設計試行 (synvelis#293)

## 問題設定

synvelis#293 の headless slice で、`crates/synvelis-player` 内に
Missing Plugin を含む typed graph を検証済み Render Substitution
と実行可能 Live Region へ分解する Resolver を実装するにあたり、
cutset validator の境界会計と plan reconciliation の安全性に
ついて 4 つの設計上の不確実点があった。

- (a) boundary output が domain/format/latency 検査に落ちた場合に
  「covered」として会計してよいか
- (b) live node の未接続入力 port を一律 dangling とみなしてよいか
  （optional `min == 0` port の扱い）
- (c) preview intent + 明示 opt-in 下で plugin state が Unknown の
  node をどう扱うか（Blocked か NonEquivalent+skip か）
- (d) Ruviform `Direction`/`SignalDomain` が `#[non_exhaustive]`
  かつ `Ord` 非実装の状況で、port layout の決定的比較をどう
  行うか

## 試行と結果

### (a) covered_sources の会計境界 — 採用: 検査通過時のみ記録

初版は boundary output edge を走査する際、domain/format/
latency の各検査が失敗して `CutsetRejection` を積んだ後でも
`covered_sources` に source endpoint を記録していた。結果、
「reject された出力が別の入力を feed している」ように見える
二重会計・dangling 判定の偽陰性が起こり得た。

`edge_ok` フラグを導入し、全検査を通過した出力のみ
`covered_sources` へ記録する形に修正した（`cutset.rs` の
`check_boundary_outputs` / `check_output_contract` 分割と共に
採用）。oracle test `ex02_*` 系で reject 理由が typed に
分離されることを確認。

### (b) optional 入力の dangling 判定 — 採用: required のみ強制

初版は region 内 live node の「feed 数 0」の入力 port を一律
`DanglingInput` として reject していたが、Send/Return fixture で
optional な key input（`multiplicity.min == 0`）が未接続の
正常系まで reject する偽陽性が出た。

`requirement_enforced()` かつ `multiplicity.min > 0` の port のみ
feed を必須とし、optional port は未接続を許容する形に修正。
一方で「region 出力境界が artifact output を持つ」要件は
optional でも維持した（出力側の網羅性と入力側の必須性は別規則）。
`ex02_send_return_boundary_input_must_be_pinned` /
`ex02_dangling_input_when_region_feed_uncovered` で両側を検証。

### (c) preview + Unknown plugin state — 採用: NonEquivalent+skip

初版テストは unknown plugin state を含む preview select に対し
`Blocked` を期待していたが、実装契約は「EditingPreview の明示
opt-in は非等価を許容し、結果に `NonEquivalent` verdict と
skipped node を記録する」設計だった。skip は
`skip_keeps_outputs_fed`（全 out-edge の行き先が fed のまま）
と required live region 非所属を条件とする fail-closed 規則で、
MainPlayback/OfflineRender/Export では一切適用されない。

テスト期待を `NonEquivalent` + skipped node 記録に修正し、
「preview が Exact を偽装しない」ことを assertion で固定した。
これは spec の「Preview は明示 Opt-in/表示」要件と整合する
判断であり、blocked に倒す案は編集プレビュー用途を過度に
制限するため棄却した。

### (d) port layout の決定的比較 — 採用: 正規化 LayoutKey

`DeclaredPort` の layout 比較に `Ord` derive を期待したが、
Ruviform `Direction`・`SignalDomain` は `#[non_exhaustive]` で
`Ord` を実装しない。`BTreeMap`/`BTreeSet` での決定的順序付けが
必要な箇所（verify の advertised layout 照合）は、数値 ordinal
+ port name に正規化した `LayoutKey` を介して比較する形にした。
`#[non_exhaustive]` への対応として wildcard arm を置き、将来の
variant 追加で panic しない構造にした。

## 結論

4 点全てについて採用案が oracle test で裏付けられ、
`SelectionVerdict::Exact` の偽装経路（未検証出力の covered
扱い・optional 入力の過剰 reject・preview の Exact 誤表示）を
塞げたことを確認した。残存リスクは `docs/impl/ir/
rcp-04-missing-plugin-resolver.md` の Risks を参照。
