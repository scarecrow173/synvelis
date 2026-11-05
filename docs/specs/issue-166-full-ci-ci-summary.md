---
id: "SPEC-034SYN166FuLlC1sTaGe"
type: "spec"
status: "proposed"
title: "Issue #166 Full CI 化・ci-summary 統一 — 安全な第一段階"
created: "2026-11-05"
updated: "2026-11-05"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/166"
    - "https://github.com/scarecrow173/ruviform/issues/365"
    - "docs/specs/issue-148-impact-classification.md"
    - "docs/specs/issue-157-selective-execution.md"
  changes:
    added:
      - type: "file"
        path: "tools/ci-summary.mjs"
      - type: "file"
        path: "tools/ci-summary.test.mjs"
      - type: "file"
        path: "docs/specs/issue-166-full-ci-ci-summary.md"
      - type: "file"
        path: "docs/designs/issue-166-full-ci-ci-summary.md"
      - type: "file"
        path: "docs/plans/issue-166-full-ci-ci-summary.md"
      - type: "file"
        path: "docs/tasks/issue-166-t1-ci-summary-stage1.md"
      - type: "file"
        path: "docs/test-specs/issue-166-ci-summary-gate.md"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "mise.toml"
    deleted:
      - type: "file"
        path: "tools/ci-impact.mjs"
      - type: "file"
        path: "tools/ci-impact.test.mjs"
      - type: "file"
        path: "tools/fixtures/run-41894049161-jobs.json"
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/issue-166-full-ci-ci-summary.md"
  depends-on:
    - "https://github.com/scarecrow173/ruviform/issues/365"
  blocks: []
  supersedes:
    - "docs/specs/issue-148-impact-classification.md"
    - "docs/specs/issue-157-selective-execution.md"
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
  superseded-by: []
  related:
    - "docs/specs/ci-synvelis-pr-push-regression-gate.md"
    - "docs/specs/issue-151-ui-ci-lane.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/issue-166-full-ci-ci-summary.md"
    - "docs/plans/issue-166-full-ci-ci-summary.md"
    - "docs/test-specs/issue-166-ci-summary-gate.md"
    - "docs/tasks/issue-166-t1-ci-summary-stage1.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-166-ci-summary-gate.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/ci-summary.mjs"
    - "tools/ci-summary.test.mjs"
  defers:
    - "pull_request/push トリガー除去(#166 変更6・Ruleset main-checkstatus required 解除後)"
    - "Scheduler/Tracker 導入(#167)"
  deferred-by: []
---

# Issue #166 Full CI 化・ci-summary 統一 — 安全な第一段階 仕様

## What Should Be Built

`ci.yml` を毎回全検証する Full CI へ移行する第一段階として、変更影響
Classifier(`classify` job・`tools/ci-impact.mjs`)と選択実行機構
(lane `if:`・`CI_SELECTIVE_DISABLE`・shadow/selective mode)を撤去し、
全 lane を全 trigger で毎回実行する。集約 gate を共通名 `ci-summary`
で新設し、`main-checkstatus` ruleset が要求する既存 required context
`evidence` は移行完了までの互換 gate として維持する。

## Why Now

- Ruviform #365 の横断移行計画で、各 repo の Full CI 全実行化 +
  `ci-summary` 統一はフェーズ1として 3 repo 並行で先行可能と決定済み。
- ただし `main-checkstatus`(ruleset id 26548278)の required context は
  実測 `evidence` であり、Owner が解除するまで `evidence` context を
  発行する job が pull_request / push で完走する必要がある。
  **PR CI トリガーだけ先に止めて required を残すデッドロックは禁止**
  (#365 決定6)なので、トリガー除去は本段階の対象外。
- Scheduler(#167 daily-ci)・Issue Tracker は未導入のため、本段階では
  移行完了を主張しない。`workflow_dispatch` + `expected_sha` 契約だけを
  先に準備し、後段の daily-ci がそのまま消費できる形にする。

## Scope

### In Scope(本段階)

1. **classify/選択実行の撤去**
   - `classify` job、`tools/ci-impact.mjs`、`tools/ci-impact.test.mjs`、
     `tools/fixtures/run-41894049161-jobs.json`、mise task
     `ci-impact`/`ci-evidence-gate`/`test-ci-impact` を削除。
   - 全 lane job の `needs: classify` と `if:` membership test を除去。
   - `CI_SELECTIVE_DISABLE` rollback variable、mode 解決
     (selective/shadow/full)、shadow 照合表、影響 DAG 分類規則を撤去。
   - 他の製品機能へ使われるコードは確認のうえ保持(影響分類の外部利用者
     は存在しないことを調査済み — tools/ 配下の CI 専用コード)。

2. **全 lane 毎回実行**
   - `verify`(ubuntu+windows)、`rt-verify`、`gw10-verify`(ubuntu+windows)、
     `msrv-verify`、`ui-verify`(ubuntu+windows)、`dep-guard` を
     pull_request/push/workflow_dispatch で毎回実行。
   - `*-macos` 専用 job(verify/gw10-verify/ui-verify)は本段階では
     維持する — `evidence` の required universe に macOS leg を戻すと
     PR gate が runner 逼迫で塞がれる #157 rev.2 の問題が復活するため。
     3OS matrix 再統合は PR トリガー除去と同時の後段で扱う。
   - RT strict test prefix・0件ガード・GW-10 harness・MSRV・SolidJS
     GUI・ruviform Git revision ガード・WebView/OS capability 分離の
     検証範囲は不変(削減禁止)。

3. **`ci-summary` fail-closed 集約(新設)**
   - job 名 `ci-summary`、全 lane job(**`*-macos` を含む**)+
     `evidence` を `needs` とし `if: always()` で必ず実行。
   - jobs API の leg 単位 conclusion を照合し、未実行・skipped・
     failure・cancelled・timed_out・unknown を成功としない。
   - **macOS leg を required として評価**する(#166 変更4: Full CI
     では macOS 失敗は Full CI 失敗)。非自動化の実機/GUI は従来どおり
     declared NotRun/Unsupported 行として記録し Pass に偽装しない。
   - Job Summary に対象 SHA・ruviform resolved SHA・job/OS・結果・
     Run URL を記録する(#365 決定3 の共通責務)。

4. **`evidence` 互換 gate(維持)**
   - required context `evidence` を死なせないため、job 名 `evidence`
     を維持し、macOS leg 以外の required leg を従来どおり fail-closed
     で照合する(macOS leg は Observed 表に記録のみ — #157 rev.2 と
     同じブロッキング境界)。pull_request/push でも完走する。
   - `evidence` は `ci-summary` の `needs` に含まれ、その失敗は
     Full CI 失敗として記録される。

5. **`expected_sha` 契約**
   - `workflow_dispatch.inputs.expected_sha`(optional string)を追加。
   - 全 job の checkout を `ref: expected_sha || github.sha` で pin。
   - GitHub は dispatch された ref の head revision(=`github.sha`)
     から workflow 定義を読み込むため、`github.sha` が workflow
     revision そのもの。`ci-summary` が
     `expected_sha == github.sha == checked-out HEAD` の 3 点一致を
     検証(指定時のみ)。不一致は fail — ref が先に進んだ stale
     dispatch は別 revision の workflow で検証した結果を候補 SHA の
     証跡と誤認し得るため拒否し、scheduler が新 head で再 dispatch
     する運用とする。
   - 各 lane の証跡は checkout 済み `git rev-parse HEAD` を記録し、
     検証対象コミットを正確に示す。
   - expected_sha・checked-out HEAD・github.sha・ruviform resolved
     SHA を `ci-summary` の Job Summary に SHA contract として記録。
   - expected_sha 未指定の手動起動・pull_request/push では既定 ref を
     検証し記録のみ行う。

### Out of Scope(後段・owner/別 issue)

- `pull_request`/`push: main` trigger の除去(#166 変更6): Owner が
  `main-checkstatus` required を空にした確認後、#145/#326 側で実施。
- `daily-ci.yml` Scheduler・`ci-issue-tracker.yml`(#167): 未導入。
- Ruleset 変更全般(`main-checkstatus` required 解除・`main-protect`
  は現行維持)。
- 3OS matrix 再統合・`*-macos` job の畳込み(PR gate 廃止と同時)。
- 旧 `10件/8件 required` 記述を含む各 Issue の履歴更新(#166 変更6
  の作業とセット、GitHub 側の編集が必要で本 worktree 範囲外)。

## Design 要点

- **gate の authoritative 実装は `tools/ci-summary.mjs`**: leg
  universe(REQUIRED_LEGS 9 + MACOS_LEGS 3 + gate job 群)を静的に
  持ち、jobs API 結果を照合する。`--mode pr` は macOS を observed
  (evidence 互換)、`--mode full` は macOS + evidence を required に
  昇格させる。分類入力を持たないため「期待集合の偽陰性」という概念
  自体が消え、skipped/unknown/未実行は常に violation。
- **placeholder の扱い**: lane `if:` が無くなったため skip-collapsed
  matrix placeholder は発生しない。placeholder 形の名前が API に現れた
  場合は契約外として unknown job violation(fail-closed)。
- **expected-jobs**: `pr` mode は 12 legs + evidence = 13 件、
  `full` mode は 12 legs + evidence + ci-summary = 14 件を下限に
  bounded retry で jobs API を収集(従来機構を踏襲)。

## Ruleset 状態(2026-11-05 実測)

- `main-checkstatus`(id 26548278): required_status_checks =
  `evidence` 1 context のみ。本段階で `evidence` job を維持するため
  required context は死なない。`ci-summary` は新規 context として
  追加発行されるが、ruleset required ではない(PR を塞がない)。
- `main-protect`(id 26530045): PR 必須・force push/削除禁止。
  変更なし。
- `pull_request`/`push: main` trigger は維持 — PR の required
  context は引き続き `evidence` で満たされる。

## Rollback

1. **本変更の revert**: `git revert` で classify/選択実行機構と旧
   `evidence` gate が一式復帰する(単一コミット/単一 PR の差分)。
   required context `evidence` は revert 前後いずれの形でも発行され
   続けるため、revert 中に ruleset 変更は不要。
2. **ci-summary が誤検出する場合**: required context ではないため
   PR gate は影響を受けない。`evidence` が従来判定を継続する間に
   修正できる。

## Acceptance Criteria(本段階)

- [ ] `node --test tools/ci-summary.test.mjs` pass(fail-closed 契約
      を pin: skipped/cancelled/failure/timed_out/unknown/absent を
      成功にしない、macOS の pr/full 昇格、unknown job 拒否、
      expected-jobs 件数)。
- [ ] `mise run lint-actions` pass。
- [ ] `mise exec -- prek run --all-files` pass。
- [ ] ci.yml に `classify`・`CI_SELECTIVE_DISABLE`・lane `if:`・
      `needs: classify` が残らず、pull_request/push/workflow_dispatch
      全 trigger で全 lane が実行される構造。
- [ ] `workflow_dispatch` に `expected_sha` input があり、lane
      checkout が pin され `ci-summary` が HEAD 一致を fail-closed
      検証する。
- [ ] job 名 `evidence` が残り required universe(macOS 以外)を
      fail-closed 照合する。job 名 `ci-summary` が全 leg(macOS
      含む)+ `evidence` を required として照合する。
- [ ] 検証範囲の縮小なし: RT 0件ガード・GW-10・MSRV・UI・dep-guard・
      declared NotRun/Unsupported 行が保持される。
- [ ] GitHub 側書き込み(push・ruleset・issue 編集)は本段階では
      行われていない。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/166
- 横断正本: https://github.com/scarecrow173/ruviform/issues/365
- 後段依存: https://github.com/scarecrow173/synvelis/issues/167
  (Scheduler/Tracker)、https://github.com/scarecrow173/synvelis/issues/145
  (Ruleset)
- 退役対象: `docs/specs/issue-148-impact-classification.md`、
  `docs/specs/issue-157-selective-execution.md`、
  `docs/specs/issue-157-matrix-placeholder-gate.md`
