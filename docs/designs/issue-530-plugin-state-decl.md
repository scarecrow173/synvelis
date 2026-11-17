---
id: "DESIGN-lc4Y6qkD6O151hJoRKMQ"
type: "design"
status: "approved"
title: "Issue #530 design — stateless/capture-applicability の契約表現"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/530"
    - "docs/specs/issue-530-plugin-state-decl.md"
    - "docs/discovery/issue-530-plugin-state-decl.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-530-plugin-state-decl.md"
  implemented-by:
    - "docs/plans/issue-530-plugin-state-decl.md"
  depends-on:
    - "docs/specs/issue-530-plugin-state-decl.md"
    - "docs/adr/issue-530-plugin-state-decl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv293-semantic-selection-integration.md"
    - "evidence/sv530/decision-ledger.md"
  refines:
    - "docs/designs/sv293-semantic-selection-integration.md"
  refined-by: []
  derives-from:
    - "docs/specs/issue-530-plugin-state-decl.md"
  derived-by:
    - "docs/plans/issue-530-plugin-state-decl.md"
    - "docs/test-specs/issue-530-plugin-state-decl.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #530 design — stateless/capture-applicability の契約表現

## 設計判断（ADR と同一結論）

**既存語彙の意味論確定（案 b）を採用する。** `PluginStateDecl` に
新 field を追加せず、`StateRestoreSupport` に variant を追加しない。

- decl 側: `PluginStateDecl::stateless()` predicate を新設し、
  stateless-by-design profile を型付きで定義する。
- env 側: `StateRestoreSupport::Unsupported` を dual-use とし、
  「plugin が state restore を実装しない」環境側事実として扱う
  （現行 doc「The plugin does not implement state restore」が既に
  この意味を持つ）。
- `verify_plugin` の `Unsupported` arm のみを変更し、
  `decl.state.stateless()` が true のとき `Live`、それ以外は従来
  どおり `StateUnsupported`。

## stateless profile の定義

`PluginStateDecl::stateless()` が true となる条件:

1. `digest.is_none()` — state blob が存在しない
2. `quality == StateCaptureQuality::Unsupported` — TIM-04 語彙の
   「plugin が state save/restore を実装しない」marker。
   `BestEffort`/`Quiescent`/`Coordinated` は capture 可能を意味し、
   `digest: None` は「capture 必要だが未 capture」となる
3. `status != OpaqueStateStatus::Known` — `Known` は検証済み opaque
   state の存在を主張するため stateless 宣言と矛盾する。
   `Unknown`/`Missing` は検証済み state を主張しないので両立する

## verdict 真理値表（verify_plugin 最終段）

| env.state_restore | decl.state | 結果 |
| --- | --- | --- |
| `Verified` | `Known` + `digest: Some` + `exact_eligible()` | `Live`（不変） |
| `Verified` | その他全て（digest None・quality 不足・status 非 Known） | `StateUnknown`（不変・fail-closed） |
| `Unsupported` | `stateless()` == true | **`Live`（新規経路）** |
| `Unsupported` | `stateless()` == false（state-bearing・未 capture・矛盾 decl） | `StateUnsupported`（不変） |
| `Unknown` | 任意 | `StateUnknown`（不変・fail-closed） |

「stateless by design」と「未 capture」の型上区別は
`StateCaptureQuality` という閉じた enum の variant で担保される:
`Unsupported` は TIM-04 が「plugin は state を持たない」と宣言する
語彙であり「capture を忘れた」意味を持たない。`digest: None` 単体は
依然として両義的だが、`quality` がそれを disambiguate する。
`PluginStateDecl` に `stateless: bool` のような並行 field を足すと
同一ドメイン事実の二重表現になり、`quality` と矛盾し得るため却下。

## 却下した代替案

### (a) `PluginStateDecl` への field/enum 追加

`capture_applicability: StateCaptureApplicability` のような新 field
を追加する案。構造体 literal 3 点が compile 破損し、うち
`crates/synvelis-render/tests/rcp07_gate.rs`:947-951 は lease 外で
scope 拡張 ask が必須。得られるのは `quality == Unsupported` と
論理的に同値な情報の別表現のみで、AC2 の「型上区別」は既存の型付き
enum で既に満たせる。冗長性と波及コストに見合わないため却下。

### `StateRestoreSupport` への「適用外」variant 追加

env 側に `NotApplicable` を新設する案。env が「plugin に state が
適用されない」事実を decl とは独立に主張できるようになるが、現行
`Unsupported`（doc: "The plugin does not implement state restore"）
が既にその env 側事実を意味しており、実供給経路
（`HostError::Unsupported → PluginStateCaptureError::Unsupported`）
も `Unsupported` に写像済み。variant 追加は `render_ui.rs` 等の
消費側波及を生み、得られる識別力はゼロ。却下。

### `PluginVerdict` への新 variant 追加

stateless 用の専用 verdict（例: `StatelessLive`）を設ける案。
`PluginVerdict` は `synvelis-workspace/src/render_ui.rs`:310-321 で
`_` arm なしの exhaustive match を受けており、lease 外の compile
破損が不可避。selection verdict 上の意味も `Live` と同一であり、
verdict 粒度を増やす利益がない。却下。

## env/decl 不一致の扱い

- env=`Verified` + decl stateless profile: 環境が restore 検証済みと
  言うのに宣言が「state 非対応」を主張 → 矛盾証拠。`Verified` arm
  は `digest.is_some()` を要求するため `StateUnknown` に fail-closed
  （`Live` にはならない）。
- env=`Unsupported` + decl が stateless profile でない
  （state-bearing・未 capture・`Known` status 併用の矛盾 decl）:
  `StateUnsupported`。いずれも「host が宣言された state 証拠を
  再現できない」ことを意味し、fail-closed を維持する。
- env=`Unknown`: stateless decl でも `StateUnknown`。
  「判定不能は fail-closed」の既存 variant 意味論を維持する。

## 波及点の確認

- `compute_structure_digest`（model.rs）は plugin declaration を
  `{:?}` で mix するが、本変更は struct 形状を変えないため digest
  値への影響は doc comment のみ（Debug derive に doc は含まれない）。
- `synvelis-workspace::rcp_selection` は `evidence.state` をそのまま
  `PluginDeclaration` に写すため、証拠構築側のコード変更は不要。
  `ProcessorPluginEvidence.state` の doc に stateless profile の
  宣言方法を明記するのみ。
- `PluginVerdict`・`StateRestoreSupport`・`PluginStateDecl` の
  variant/field 数は不変のため、`render_ui.rs` の exhaustive match・
  `rcp07_gate.rs` の構造体 literal・`rcp06_render_ui.rs` は無変更で
  compile が維持される。
