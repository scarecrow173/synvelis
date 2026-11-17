---
id: "DESIGN-034YXXUP2FqWQZDrCaOwUz"
type: "design"
status: "approved"
title: "SV177 design tokens — canonical vocabulary mapping, parity evidence, screen fixtures"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/177"
    - "docs/discovery/issue-177-design-tokens.md"
    - "docs/specs/issue-177-design-tokens.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-177-design-tokens.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/issue-177-design-tokens.md"
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-177-design-tokens.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/177"
    - "https://github.com/scarecrow173/ruviform/issues/385"
  defers: []
  deferred-by: []
---

# SV177 design tokens — canonical vocabulary mapping, parity evidence, screen fixtures

## Summary

出荷 DTCG 文書に canonical semantic 語彙を alias 層として追加し、
`ResolvedTheme` → `ViewPalette` の parity を headless/gpui-test の
契約テストで固定する設計。`ViewPalette` が private である制約から、
parity は「resolved 値が `from_theme` の read key を全て製品値で満たし、
fallback literal と一致しない」ことの ResolvedTheme レベル証明 +
gpui-test lane での `theme: Some(..)` mount 受理で構成する。

## System Context and Problem Boundaries

- `ruviform-ui-theme`（pin `cbcc834`）が DTCG 文書の解釈・alias・
  mode/density merge・`ResolvedTheme` 生成の唯一の権威。
  Synvelis 側 `synvelis-theme` は文書 envelope・catalog・preferences・
  adapter のみを持ち、token 解釈を再実装しない。
- `ruviform-ui-gpui` の `ViewPalette::from_theme(&ResolvedTheme)` が
  canonical 語彙を read する唯一の consumer。`ViewPalette` 自体は
  private module（`render.rs`）で、公開 re-export はない。
  `ViewMount { host, theme: Option<ResolvedTheme> }` が公開 mount 点で、
  `theme: Some(..)` は内部で必ず `from_theme` を通る。
- 出荷文書の現行語彙は `color.*`/`font.*`/`spacing.*`/
  `control.height`/`radius.*`/`motion.*`。canonical 語彙は未定義で、
  `ViewPalette` は全項目 fallback literal に落ちている。

## Scope

### In Scope

- mode 文書（dark/light）への canonical color group 追加:
  `surface.{bg,raised,border}`、`text.{primary,dim}`、
  `accent.primary`、`state.{error,disabled}`、`control.{track,fill,
  thumb}`、`viz.{waveform,waveform_rms,baseline,cursor}`。
- density 文書への `space.{sm,md,page}` 追加（`ViewPalette` 消費は
  `space.sm`/`space.md`、`space.page` は層内補助）。
- `assets/themes/*.dtcg.json` 5 文書の alias/直接値設計。
- `crates/synvelis-theme` contract test（canonical key が製品値を
  返す・fallback literal と一致しない・4 組合せ）。
- `crates/synvelis-app/tests/theme_parity.rs`（gpui-test lane）。
- 画面別 semantic component key ↔ canonical token fixture と
  2scale×2theme 意味論差分の evidence 記録。
- provisional 値注記の更新（light `bg-active`/`bg-input`、
  `state.disabled`、`viz.waveform_rms`、status ramp）。

### Out of Scope

- `ViewPalette`/upstream ruviform の変更、`dock.*` 語彙、
  GPUI 製品画面本体、実 OS window/pixel golden、Workspace Preset
  製品配線、Plugin GUI 内部、docs index、Issue close。

## Components and Boundaries

| Component | Responsibility |
| --- | --- |
| `assets/themes/synvelis-{dark,light}.dtcg.json` | mode 層。canonical color group を `color.*` primitive への alias + 直接値で定義 |
| `assets/themes/synvelis-density-{compact,comfortable}.dtcg.json` | density 層。canonical `space.*` を `spacing.*` への alias で定義 |
| `assets/themes/synvelis-base.dtcg.json` | base 層。変更なし（font/radius/motion/easing は既存語彙で完結） |
| `crates/synvelis-theme` tests | canonical vocabulary・4 組合せ解決・fallback 非一致の headless 契約 |
| `crates/synvelis-app/tests/theme_parity.rs` | gpui-test lane の parity 証拠（mount 受理・fixture 対応・意味論差分） |
| `evidence/sv177/` | parity/差分/NotRun 記録 |

## Data and Control Flow

1. `ThemeCatalog` が 5 文書を load（base → mode → density の layer
   merge は upstream interpreter が実施）。
2. `resolve(theme, mode, density)` → `ResolvedTheme`。canonical key
   は alias 解決済みの `ResolvedColor`/`ResolvedLength` として露出。
3. `ViewMount { theme: Some(resolved) }` → `ViewPalette::from_theme`
   が canonical key を read。欠落 key は `fallback()` literal。
4. parity 証拠: (a) ResolvedTheme 上で canonical key が製品値を持つ
   ことを assert、(b) gpui-test で mount → settle → drawn plan を
   取得し theme 受理を確認、(c) fixture 対応表と 2scale×2theme 差分を
   evidence に記録。

## Canonical Token Mapping Design

`ViewPalette::from_theme`（pin `cbcc834`）が read する key と、製品
文書での定義方法:

| canonical key | type | 定義方法 | alias 先 / 直接値 |
| --- | --- | --- | --- |
| `surface.bg` | color | alias | `{color.bg}` |
| `surface.raised` | color | alias | `{color.bg-panel}` |
| `surface.border` | color | alias | `{color.border}` |
| `text.primary` | color | alias | `{color.fg}` |
| `text.dim` | color | alias | `{color.fg-dim}` |
| `accent.primary` | color | alias | `{color.accent}` |
| `state.error` | color | alias | `{color.err}`（chrome error primitive。status ramp `color.status-*` とは別契約で流用しない） |
| `state.disabled` | color | 直接値 | provisional: dark `#4e565f` / light `#9aa0a8`（design review で確定） |
| `control.track` | color | alias | `{color.bg-input}` |
| `control.fill` | color | alias | `{color.accent}` |
| `control.thumb` | color | alias | `{color.fg}` |
| `viz.waveform` | color | alias | `{color.accent}` |
| `viz.waveform_rms` | color | 直接値 | provisional: dark `#8fb8e8` / light `#7aa8d8`（design review で確定） |
| `viz.baseline` | color | alias | `{color.border}` |
| `viz.cursor` | color | alias | `{color.fg}` |
| `space.sm` | dimension | alias | `{spacing.sm}` |
| `space.md` | dimension | alias | `{spacing.md}` |
| `space.page` | dimension | alias | `{spacing.lg}`（density 層の補助 token。`ViewPalette` 未消費） |

上表は review-protocol 通過後の確定値。`state.error` は status ramp
（`color.status-*`）とは別契約であり、chrome 状態の error 表現とし
て既存 `color.err` primitive を alias 先に採用した。
`state.disabled`/`viz.waveform_rms` は対応する `color.*` primitive
が存在しないため mode 別の直接値とし、provenance に provisional
注記を付す。

### light theme の provisional 補完

light 文書は `color.bg-active`/`color.bg-input` を欠く。canonical
alias（`control.track`→`bg-input`）が解決できるよう、両 token を
provisional 値で補完する: `bg-input`=`#eaecf0`、`bg-active`=
`#e0e4e9`（dark の recessed/pressed 階層に対応する light 側の暫定
値）。provenance 注記に「provisional — native design review
（RF#631/SV#114）で確定」と明記する。

## Parity Evidence Design

`ViewPalette` は private のため、palette 内部値の直接照合は不可能。
以下の 3 層で「ResolvedTheme → ViewPalette に製品差分が実際に投影
される」ことを契約固定する:

1. **Headless resolved-value assert**（`synvelis-theme` tests）:
   4 組合せで `theme.color("surface.bg")` 等の canonical key が
   `Err`/fallback ではなく製品値を返し、値が `ViewPalette::fallback`
   の literal と一致しないことを検証。fallback literal は pin rev の
   `render.rs` から転記した定数として test 内に保持する
   （upstream 変更時は test 更新で検出）。
2. **gpui-test mount 受理**（`synvelis-app/tests/theme_parity.rs`）:
   `ViewMount { host, theme: Some(resolve(..)) }` で main screen
   spec を mount → `gui.settle()` → `observe`/`drawn` が空でない
   plan を返すことを assert。`theme: None`（fallback）との描画 plan
   差分は色を含まないため観測不能 — この限界を test コメントと
   evidence に明記する。
3. **画面 fixture + 意味論差分**: `screens.rs` の semantic component
   key 一覧を fixture として固定し、各画面が消費する canonical token
   期待値を対応表化。2scale（`scale_factor` 1.0/2.0）×2theme
   （dark/light）で resolved token 値の差分を evidence に記録
   （px unit のため scale 次元の値差はなく、差分は theme mode と
   density のみ — この事実自体を記録する）。

## Risks and Trade-offs

| Risk | Mitigation |
| --- | --- |
| alias 先の `color.*` 値が semantic 的に不適切 | design review で確定。provisional 注記で区別 |
| `ViewPalette` private により parity が ResolvedTheme レベルに留まる | upstream public 化は deferred。ResolvedTheme assert + mount 受理で契約を固定 |
| density の `spacing.*` と canonical `space.*` の二重語彙 | `space.*` は `spacing.*` への alias とし density 差分を継承。`spacing.*` は既存 consumer 互換のため残置 |
| `state.disabled`/`viz.waveform_rms` の直接値が暫定 | provisional 注記 + design review gate を文書化 |
| 既存 `color.*` consumer（web/native）との整合 | `color.*` は残置し canonical は追加層。rename は行わない |

## Deferred Design Concerns

- `ViewPalette` public 化・palette 色 observer: upstream RF issue 化
  が必要になった時点で再検討。
- `dock.*` 語彙: pin rev が未消費。SV#176 で必要になった時点で追加。
- status ramp 色値確定: RF#631/SV#114 design review。

## Verification Notes

- [ ] `mise run test synvelis-theme` — canonical vocabulary 契約
- [ ] `mise run test-app-gpui synvelis-app`（または同等 task）—
  parity mount テスト
- [ ] `mise run fmt-check` / `check`/`lint` 対象 package
- [ ] `mise run check-headless-deps` — 依存追加なしの確認
- [ ] review-protocol による design レビュー（blocker なし）
- [ ] evidence/sv177/ に parity/差分/NotRun 記録
- [ ] 実 OS window/pixel golden は NotRun（RF#631+SV#114 gate）

## Review Notes

review-protocol（2026-11-17、Coordinator 契約により subagent なしの
分離 pass で実施 — Reviewer A: 仕様整合、Reviewer B: 敵対的検証、
Validator/Judge 統合）:

- Reviewer A: alias 先 `{color.bg-raised}` は存在しない primitive
  だった → `{color.bg-panel}` に修正（CONFIRMED P1、修正済み）。
  `state.error` の alias 先未確定 → `{color.err}` に確定（status
  ramp との混同を回避）。
- Reviewer B: `control` group 名が density 文書の `control.height`
  （dimension）と mode 文書の `control.*`（color）で共有される件を
  検証 — upstream parser は `$type` を group 内で継承し token path
  単位で merge するため競合しないことを `doc.rs` で確認（棄却）。
  compact `space.sm`=4.0 が fallback literal と一致する件 —
  「fallback 非一致」ではなく「期待製品値との一致 + 全 key 解決」
  を契約とし、一致は偶然である旨を test/evidence に記録（対応済み）。
- 判定: CONFIRMED P0/P1 なし。P2/P3 の指摘は上記のとおり処理済み。
  blocker なし → `status: approved`。
