---
id: "ADR-JlTrO2yuccd5FZv5Nn49fc"
type: "adr"
status: "accepted"
title: "Issue #516 — interim synvelis-script-luau(mlua) を ruviform-script/-luau 製品 port への薄い binding 交換で置換"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/516"
    - "https://github.com/scarecrow173/synvelis/issues/271"
    - "https://github.com/scarecrow173/synvelis/issues/272"
    - "https://github.com/scarecrow173/synvelis/issues/273"
    - "docs/discovery/issue-516-mlua-adapter.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-script-luau/src/lib.rs"
      - type: "file"
        path: "evidence/sv516/binding-fit.md"
    modified:
      - type: "file"
        path: "crates/synvelis-script/src/backend.rs"
      - type: "file"
        path: "crates/synvelis-script-luau/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-script-luau/tests/backend.rs"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-516-mlua-adapter.md"
  implemented-by:
    - "docs/designs/issue-516-mlua-adapter.md"
  depends-on:
    - "docs/specs/issue-516-mlua-adapter.md"
    - "docs/designs/issue-516-mlua-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-script-03-luau-backend.md"
    - "docs/specs/sv-script-03-luau-backend.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-516-mlua-adapter.md"
  derived-by:
    - "docs/designs/issue-516-mlua-adapter.md"
  verifies: []
  verified-by: []
  references:
    - "evidence/sv516/binding-fit.md"
    - "https://github.com/scarecrow173/ruviform/issues/560"
    - "https://github.com/scarecrow173/ruviform/pull/729"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# Issue #516 — interim `synvelis-script-luau`(mlua) を ruviform 製品 port への薄い binding 交換で置換

## Context and Problem Statement

Synvelis#273 で導入した interim Luau backend（`synvelis-script-luau`、
mlua 0.12.2 vendored Luau を crate 内に直接実装）は、RF-SCRIPT-00
consumer-side port（`synvelis-script::backend`）を満たす暫定実装
だった。Ruviform 側では RF-SCRIPT-00 中立契約
（`ruviform-script`）とその製品 Luau backend
（`ruviform-script-luau` — vendored Luau.Analysis 0.740 + mlua
0.12.2 Rust API、PR `scarecrow173/ruviform#729` merge commit
`dce45712`）が main に着地済みであり、Synvelis#516 は
「interim adapter を恒久採用するか、製品 port を消費する形へ
交換するか」を確定する decision + 実装 slice である。

## Decision Drivers

- **重複実装の解消**: interim は sandbox globals 剥奪・lift 規則・
  budget・digest・artifact provenance を synvelis 側に複写しており、
  製品 backend が同じ責務を上流に持つ。二重保守は drift の温床。
- **consumer 契約の非破壊**: `synvelis-cli`/`synvelis-automation` は
  `synvelis_script::backend` port 経由で消費しており、公開面を
  維持できれば consumer 修正は不要（sv#515 が cli を占有中のため
  lease 上も望ましい）。
- **fail-closed の維持**: source digest pin・unsupported budget・
  言語不一致・単一 artifact・single-shot evaluate の各 fail-closed
  契約を退行させない。
- **headless 境界**: mlua 依存は backend crate に隔離し、consumer
  （cli/automation/gateway）は port trait のみを消費する既存契約を
  維持する。
- **正直な provenance**: artifact の `produced_by.impl_version` は
  実際に VM を走らせた実装（製品 backend）を名乗るべき。

## Considered Options

### A. interim mlua adapter の恒久採用（docs-only 分岐）

interim 実装をそのまま残し、ADR で採用根拠と移行条件を記録する。

- 長所: 変更ゼロ・consumer 無影響・build が軽い（C++ Analysis
  shim を引かない）。
- 短所: 製品 port が存在するのに consumer-side 複写を恒久化 —
  RF-SCRIPT-00 の「consumer は独立 port を消費する」契約（
  `scarecrow173/ruviform#560`）の精神に反し、sandbox/budget/lift
  の二重保守が残る。「once RF#559 lands」の stale 記述が示す通り
  元々は交換前提の暫定実装。

### B. synvelis port 語彙を ruviform 契約型へ再 binding（mlua impl は残す）

`synvelis-script` の公開面を ruviform 型へ寄せ、interim mlua 実装を
rf trait 実装へ書き換える。

- 長所: 製品契約語彙への早期整合。
- 短所: consumer 公開面を破壊するため cli/automation の修正が必須
  （lease 外・sv#515 占有中）。かつ mlua sandbox/lift/budget の
  複写は残り、重複解消という主目的を達しない — 最悪の両取り損。

### C. `ruviform_script_luau::LuauBackend` を消費する薄い binding（**採用**）

`synvelis-script-luau` を anti-corruption adapter へ書き換え、
synvelis port 公開面（`ScriptBackend`/`ScriptSession`/全 vocabulary）
を維持したまま内部で製品 backend へ委譲する。

- 長所: 重複実装を上流へ一元化・consumer ゼロ変更・port 契約
  （digest tag・error 分類・artifact provenance）を binding で
  正直に維持・mlua は上流 crate に隔離されたまま。
- 短所: 語彙差分の写像層が必要（本 ADR の binding-fit 表に網羅）。
  `ruviform-script-luau` が vendored Luau.Analysis C++ shim を
  build に引くため build 時間が増える。workspace の ruviform pin が
  `cbcc8343` → `84a68f25` へ進む（旧 pin には同 crate が存在しない
  ため必須）。

## Decision Outcome

**Option C を採用** — `synvelis-script-luau` を ruviform 製品
backend への薄い binding へ置換した。

### 決定の根拠

1. **直接交換は不可能だが binding 交換は成立**: exhaustive
   binding-fit 照合（`evidence/sv516/binding-fit.md`）で、trait
   署名・error enum・digest 形式・SessionConfig 形状が全面的に
   異なり crate 単位の drop-in は成立しない一方、synvelis port
   公開面を維持する adapter で全差分を吸収できることを確認した。
2. **consumer 影響ゼロを実測**: `synvelis-automation` 全 test
   （`.luau` real-VM resolver/eval 含む）green、`synvelis-cli`
   check 通過 — lease 外ファイルの変更は不要だった。
3. **fail-closed 契約の維持**: adapter が open 時検証を interim
   順序で先行実施し、`UnsupportedArtifact`・`UnsupportedBudget`
   （`HostCall` 含む）・digest tag 検証・evaluate-once
   `ContractViolation`・close 後 `Backend` を全て保持した。
4. **port 語彙の正直な拡張は最小限**: `ScriptErrorKind::
   BudgetExceeded.kind` を `Option<BudgetKind>` へ変更したのは、
   中立 `ScriptError` が次元情報を運ばず複数 budget 併用時に
   帰属不可能なため — `None` で「不明」を正直に返す。
   `synvelis-script` の他の port 面（action/workflow/schema/model）
   には触れていない。
5. **analyzer は対象外**: `LuauAnalyzer`（native Luau.Analysis
   bridge）への切替は sv#274 の gate であり本 slice に含めない
   （`LuauPortAnalyzer` は interim port のまま・回帰なし）。

### binding が引き受けた差分（要点）

- source digest: consumer の `sha256:<hex>` tag を open 時に検証、
  製品側 `sha256:v1:` digest は `ScriptSource::new` が自動計算。
- artifact digest: 製品 `DefinitionDigest` は使わず、interim と
  同一式 `digest_of(to_json(payload))` で synvelis canonical の
  digest を再計算（consumer が再計算可能な形式を維持）。
- `produced_by.impl_version`: 製品 backend descriptor の値を記録
  （実装 provenance を正直に）。
- error 分類: `rf Internal` → synvelis `Internal`（1:1）。
  lifted-shape 違反（multi-return・sparse/mixed table・depth/
  node 上限・非 record artifact payload）は製品側 `Internal` —
  interim の `ContractViolation` とは分類が変わる（consumer は
  区別していない）。`ContractViolation` は binding 検出違反と
  port 契約違反（2 回目 evaluate 等）に限定。
- `BudgetExceeded.kind`: 単一 armed budget 次元のみ `Some`、
  複数併用時は `None`（中立 error が次元を運ばないため推測しない）。

### 移行条件・残件（将来の再評価トリガ）

- **ruviform port の成熟度**: host binding handler seam・named
  entry・instruction budget・Resume VM が中立契約に追加された
  場合、binding の写像層を再評価する。現時点では全て
  「中立契約に表現なし」として記録済み（ruviform Issue 起票の
  要否は PR 本文に記録 — host seam は Synvelis#261/#267 系の
  authority 契約と絡むため consumer 側設計が先）。
- **sv#274（SV-SCRIPT-04）**: 公式 Luau.Analysis native bridge
  接続・Script Editor 製品化の gate。本 slice の runtime port
  交換と独立だが、analyzer を `LuauAnalyzer` へ切り替える際は
  `synvelis-automation/src/analyzer.rs` の port adapter が対象
  になる見込み。
- **sv#267（AUTX-08）**: script 経路の最終 E2E・実 OS 受入の
  結合先。3-OS（macOS/Linux）実機受入は未実施 — ruviform 側も
  Windows `x86_64` のみ実測であり、consumer gate として NotRun
  を維持する。
- **ruviform pin**: workspace 全 ruviform crate が
  `cbcc8343` → `84a68f25` へ更新された（`ruviform-script-luau`
  が旧 pin に存在しないため必須）。workspace 全体の consumer
  影響は Daily Full CI の検証に委ねる。

## Consequences

- interim mlua 実装（sandbox globals・lift・budget・digest・
  artifact provenance の synvelis 側複写）は撤去され、上流
  `ruviform-script-luau` が唯一の VM 実装になった。
- `synvelis-script-luau` は VM を持たない薄い binding となり、
  責務は語彙写像と fail-closed 事前検証に限定された。
- `Cargo.lock` の ruviform 解決 rev が `84a68f25` に更新
  （git 依存の既定ブランチ追従どおり）。
- `BudgetExceeded.kind` が `Option<BudgetKind>` になった —
  consumer は `kind` を `BudgetKind` として直接使えない
  （現行 consumer は match していないため影響なし）。
- 実 Luau.Analysis native FFI bridge 経由の analyzer 走行は
  sv#274 gate として **NotRun** のまま — 本 slice の headless
  Pass を native analyzer Pass に昇格しない。
