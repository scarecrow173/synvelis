---
id: "TSPEC-034XKoUXUzjsrGD2SvedHg"
type: "test-spec"
status: "approved"
title: "AUTX-07 theme status tokens + Script Editor parity test spec（synvelis#266 SV266-NR-02/03）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/266"
    - "https://github.com/scarecrow173/synvelis/issues/274"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/sv266_parity.rs"
      - type: "file"
        path: "docs/impl/ir/autx-07-theme-parity.md"
      - type: "file"
        path: "docs/impl/exp/autx-07-theme-parity.md"
      - type: "file"
        path: "docs/test-specs/autx-07-theme-parity.md"
      - type: "dir"
        path: "evidence/sv266/"
    modified:
      - type: "file"
        path: "assets/themes/synvelis-dark.dtcg.json"
      - type: "file"
        path: "assets/themes/synvelis-light.dtcg.json"
      - type: "file"
        path: "crates/synvelis-theme/tests/theme_contract.rs"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/test-specs/autx-07-gui-views.md"
    - "docs/test-specs/sv-script-04-script-editor.md"
    - "docs/test-specs/sv177-theme-preset-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-07-gui-views.md"
    - "docs/test-specs/sv-script-04-script-editor.md"
    - "docs/test-specs/sv177-theme-preset-contract.md"
    - "docs/impl/ir/autx-07-theme-parity.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/designs/sv177-product-theme-workspace-preset-design.md"
  derived-by: []
  verifies:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/designs/sv177-product-theme-workspace-preset-design.md"
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/267"
  defers: []
  deferred-by: []
---

# AUTX-07 theme status tokens + Script Editor parity test spec（synvelis#266 SV266-NR-02/03）

## Purpose

`evidence/sv266/README.md` §3 の Deferred Case のうち 2 件を固定する:

- **SV266-NR-03**: `assets/themes/synvelis-{dark,light}.dtcg.json` の
  `color` group に `StatusTone::ALL` 全 path の `status-*` `$value` を
  追加し、`crates/synvelis-theme/tests/theme_contract.rs` が mode×density
  全 4 組合せでの resolve を保証する。
- **SV266-NR-02（部分充足）**: Action Palette 側
  （`project_action_row`/`resolve_capability`）と Script Editor 側
  （`project_script_api`/`ScriptApiModel`）が**同一
  `CapabilityDescriptor`/`RegistrySnapshot` を唯一の source** として
  同じ capability/schema/availability verdict を示すことを、
  `crates/synvelis-workspace/tests/sv266_parity.rs` で契約レベル照合
  する。analyzer PASS が Gateway 認可 PASS を意味しないことを
  headless fixture で固定する。

実 GPUI OS/pixel/IME/DPI gate（SV266-NR-01）と Script Editor surface
本体の実装は本 spec の対象外であり、headless Pass を実機 Pass に
昇格しない。

## Feature

Theme status token resolution と Action Palette ⇔ Script Editor の
capability parity projection。

## Rules

- **Rule AUTX-THEME-STATUS-TOKENS** 出荷 theme 文書の `color` group は
  `StatusTone::ALL` の全 `token_path()`（`status-*`）を `$value` として
  持ち、`theme.color("color.{path}")` が mode（dark/light）× density
  （compact/comfortable）の全 4 組合せで resolve する。vocabulary の
  正本は `synvelis_theme::status::StatusTone` であり、test は `ALL`
  を走査する — tone 数を literal で assert しない。
- **Rule AUTX-THEME-MODE-OWNED** `color` token は mode 文書
  （dark/light）にのみ置く — base/density 文書に `color` group を
  追加しない（`docs/designs/sv177-product-theme-workspace-preset-design.md`
  の文書層規約: base = mode/density 非依存 token）。density 選択は
  color を変えない。
- **Rule AUTX-PARITY-SAME-SOURCE** 両 surface の capability 解決は
  同一 `RegistrySnapshot.capabilities`（= `&[CapabilityDescriptor]`）
  を source とする。`ScriptApiRow.capability`（requested
  `id@version`）を `resolve_capability` に通した結果と、model entry
  の `descriptor` は同一 descriptor を指す。
- **Rule AUTX-PARITY-FAILCLOSED** exact ref が snapshot に存在しない
  場合（version 不整合・未登録 id）、script 側 verdict は
  `unsupported-version`/`unknown-capability` となり palette 側
  `resolve_capability` は `UnknownCapability` — 両側とも fail-closed
  で一致し、実行可能表示を生まない。
- **Rule AUTX-PARITY-AVAILABILITY** script verdict label
  （`invocable`/`disabled`/`planned`/`retired`）は同一 descriptor の
  `Availability::name()` と整合する。`ActionRow.executable` は
  `availability == Available` ∧ `grant.satisfied()` のときのみ true
  （SV266-AC-01 と同一契約を本 fixture で再確認）。
- **Rule AUTX-PARITY-SCHEMA** descriptor が解決した entry では、
  script 側 `SchemaBinding.schema`（input/output）が palette 側
  `ActionRow.input_schema`/`output_schema` と同一 `SchemaRef` を指し、
  `ActionRow::schema_label` の `name@version` 表示と一致する。
- **Rule AUTX-PARITY-TARGET** `ActionRow.target`/`domain`/`effects` は
  descriptor の verbatim 投影であり、script 側 `descriptor` が保持する
  同一値と一致する（投影が権威値を改変しない）。
- **Rule AUTX-ANALYZER-NOT-AUTHORITY** `ScriptAnalyzerStatus` の全
  variant（`Idle`/`Ready`/`Dirty`/`Stale`/`Unavailable`/`Unsupported`/
  `Rejected`）の存在は `ActionRow.executable`・`ScriptApiRow.verdict`
  を一切変化させない。analyzer PASS は Gateway 認可 PASS を意味
  しない（Issue #266 受入 1・2 行目）。
- **Rule AUTX-PARITY-HIDDEN** `ScriptVisibility::Hidden` の entry は
  `project_script_api` の public rows に現れず、palette 側 catalog
  （descriptor 列）にも script 名として混入しない。

## Examples

- **Example AUTX-THEME-STATUS-TOKENS-01** `StatusTone::ALL` を走査し、
  `color.status-ok` … `color.status-diagnostic-error` の 17 path が
  dark/light × compact/comfortable の 4 resolve 全てで `Some` を返す。
  density ペア内では同一 mode で値が一致し、dark/light 間では ramp
  全体として異なる値を持つ。
- **Example AUTX-PARITY-SAME-SOURCE-01** `synvelis.midi.add-note@1.0`
  を指す surface entry は verdict `invocable`、model `descriptor` と
  `resolve_capability(snapshot.capabilities, "synvelis.midi.add-note@1.0")`
  が同一 `CapabilityDescriptor` に解決され、`ActionRow.capability_label()`
  が `synvelis.midi.add-note@1.0` に一致する。
- **Example AUTX-PARITY-FAILCLOSED-01** `synvelis.midi.add-note@9.9`
  （version 不整合）は script verdict `unsupported-version`、palette
  `resolve_capability` は `UnknownCapability`。`synvelis.query.
  nonexistent@1.0`（未登録 id）は `unknown-capability` /
  `UnknownCapability` で一致する。
- **Example AUTX-PARITY-AVAILABILITY-01** fixture の `disabled`/
  `planned`/`retired` descriptor は script verdict が同名
  availability を示し、grant `Satisfied` でも `executable=false`。
  `Available` + grant `Missing`/`Unknown` も `executable=false`。
- **Example AUTX-ANALYZER-NOT-AUTHORITY-01** `Available` capability に
  対し grant `Missing` の `ActionRow` は analyzer `Ready` の有無に
  関わらず `executable=false`・`blocked=Some` のまま。editor document
  の status が `Idle→Ready→Dirty→Stale→Unavailable→Unsupported→
  Rejected` と遷移しても `doc.api()` の verdict rows は不変。

## Guarantees

- `StatusTone` vocabulary に将来 tone が追加されても、theme test は
  `ALL` 走査により自動で新 path の resolve を要求する（件数固定の
  assert は置かない）。
- parity test は `synvelis-workspace`/`synvelis-script`/
  `synvelis-gateway` の public API のみを使い、private 実装詳細や
  src 側契約への変更を必要としない。src 側に契約 bug を発見した場合は
  test を弱めず Fail として記録する。
- 両 surface が同一 descriptor source を共有することが fixture 上で
  再確認され、片側のみ実行可能に見える表示経路が headless 契約上
  存在しないことが固定される。

## Non-goals

- 実 GPUI OS/pixel/IME/DPI/2Window/WindowClose・Alert・Undo/Save/
  Reload の実機受入（SV266-NR-01 → synvelis#114/#267 別 Gate、
  **NotRun**）。
- Script Editor surface 本体・Analyzer/Diagnostics/Completion の
  実装変更（synvelis#274 Owner の領域）。
- `assets/themes/` への新規文書追加・envelope 構造変更・base/density
  文書への `color` group 追加。
- `synvelis_theme::status::StatusTone` vocabulary 自体の変更
  （17 tone は確定済み正本）。
- live adapter 接続（SV266-NR-04）・composition mount 配線
  （SV266-NR-05）・gpui-test scenario（SV266-NR-06）。
- `synvelis_gateway::fixture::fixture_descriptors` との突合は補助的
  であり、parity の source は `RegistrySnapshot` fixture に固定する。

## Risk

- 本 spec を外すと、`status-*` token が theme 文書に未定義のまま
  surface 側で参照される退行、および Action Palette と Script Editor
  が別々の capability 解釈で「実行可能」「invocable」を表示する
  二重正本を検出できなくなる。
- analyzer PASS を実行認可 PASS と混同する UI 文言・状態遷移が
  headless 契約で固定されないまま残る。
