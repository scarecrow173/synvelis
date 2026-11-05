---
id: "TSPEC-034SVCIOPT02gT5yUiOp"
type: "test-spec"
status: "approved"
title: "Issue #148 変更影響分類・Fail-Closed evidence Gate 検証仕様"
created: "2026-11-04"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/148"
    - "docs/specs/issue-148-impact-classification.md"
  changes:
    added:
      - type: "file"
        path: "tools/ci-impact.mjs"
      - type: "file"
        path: "tools/ci-impact.test.mjs"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-148-impact-classification.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-148-impact-classification.md"
    - "docs/plans/issue-148-impact-classification.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-148-impact-classification.md"
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "https://github.com/scarecrow173/synvelis/blob/3f4f4450131b0258a1919b213de3ad394fbbb709/tools/ci-impact.mjs"
    - "https://github.com/scarecrow173/synvelis/blob/3f4f4450131b0258a1919b213de3ad394fbbb709/tools/ci-impact.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #148 変更影響分類・Fail-Closed evidence Gate 検証仕様

## Purpose

Issue #148 が要求する「必要な検証漏れなし・skipped を Pass にしない
fail-closed 集約」不変条件を pin する。分類規則や lane 構成が将来
変わっても、ここに挙げる保証は維持されなければならない。

## Feature

変更ファイル → impact class → crate 依存伝播 → 期待 lane 集合を決める
`tools/ci-impact.mjs classify` と、期待集合 vs Actions run 実結果を
leg 単位で照合する `tools/ci-impact.mjs gate`(集約 `evidence` job)。

## Rules

- Rule 1: 不明・分類不能・共有依存/設定変更は Full CI fallback。
  検証集合は過小にならない。
- Rule 2: `skipped`/`cancelled`/`failure`/未実行・leg 欠落は required
  leg で常に violation。GitHub required checks の skipped=成功
  セマンティクスに依存しない。
- Rule 3: selective mode では `not-applicable` 宣言 leg の skipped/未実行
  のみ許容。根拠なし skip・required leg skip・NotApplicable leg の実
  失敗(偽陰性)は violation。
- Rule 4: lane universe は ci.yml job 名に total。未知 job は violation。
- Rule 5: RT owner affected → `rt-verify`、GW owner affected →
  `gw10-verify` ×3、任意 crate changed/affected → `verify` ×3 +
  `msrv-verify`、solidjs-gui → `ui-verify` ×3、非 docs → `dep-guard`。
- Rule 6: PR は shadow mode で全 lane 実行(skip 無効)。main
  push/dispatch は full mode。
- Rule 7: impact report 欠落・parse 不能は gate fail(分類不能 run を
  緑にしない)。
- Rule 8: `classify` job は分類前に `test-ci-impact` 自戒テストを実行。
- Rule 9: 証跡は head/base/ruviform SHA・mode・leg 単位 expected/actual/
  verdict・Shadow 照合・Declared NotRun/Unsupported・Runner 時間を含む。

## Examples

- `docs/**` のみ変更 → 期待集合 = `classify`(+`evidence`)。dep-guard も
  NotApplicable。
- `crates/synvelis-recording/**` 変更 → verify×3 + rt-verify +
  msrv-verify + dep-guard + classify 必須。gw10/ui は NotApplicable。
- `crates/synvelis-project/**` 変更 → 上記 + gw10-verify×3(gateway
  dep 伝播)。
- `ui/**` のみ変更 → ui-verify×3 + dep-guard + classify。
- `Cargo.lock`/`mise.toml`/`.github/**`/未知 path → 全 lane 必須。
- ある leg が `skipped` で終了 → `evidence` fail(shadow mode では
  全 leg required のため)。NotApplicable leg が実 fail → 偽陰性
  violation。

## Guarantees

- 分類の誤り(過少見積)は Shadow 照合で可視化され、selective 未導入の
  現状では merge block 力を持たない。
- lane skip が将来導入されても、根拠付き NotApplicable 以外の未実行は
  `evidence` が必ず検出する。
- `branding` 等の universe 外 context は gate の lane 判定に混入しない
  (unknown job 検出は run 内 job に限る)。

## Non-goals

- lane `if:` 条件実行そのものの検証(selective 有効化は別変更)。
- Ruleset required context 構成の検証(本変更は構成を変えない)。
- Runner 時間 p50 の計測保証(CI-OPT-00 基準の Before/After は PR
  証跡で記録)。

## Risk

- LANE_LEGS が実 job 名とずれた場合 gate が誤判定する → 未知 job は
  violation、leg 欠落も violation なので両方向とも fail(緑誤判定は
  起き得ない)。
- jobs API が古い attempt を返す → `run_attempt` フィルタで対応済み。
