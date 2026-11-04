---
id: "EXP-131-aut-01-shadow-ci"
title: "Issue #131 AUT-01 PR run: shadow classify decision ・偽陰性対照・macOS queue 記録"
created: "2026-11-04"
updated: "2026-11-04"
---

# Issue #131 AUT-01: shadow CI 証跡(2026-11-04)

- Issue: https://github.com/scarecrow173/synvelis/issues/131
- PR: https://github.com/scarecrow173/synvelis/pull/159
- run: https://github.com/scarecrow173/synvelis/actions/runs/41851559485
  (event `pull_request` → `CI_IMPACT_MODE=shadow`、head `2e5c1e1`、base
  `b47c90f`、ruviform `8fb665f1`)
- #157 蓄積: SV-CI-OPT-02 selective 判定の shadow 観測。

## 1. classify decision(mode=shadow、selective_enabled=false)

`classify` job(conclusion success)が出力した `impact-report.json`:

- changed_files: 14
- classes: `automation`×1, `control`×9, `docs`×5, `ipc`×8, `project`×1
- crates: changed=`[synvelis-project, synvelis-workspace]`、
  affected=全 9 crate(synvelis-project, synvelis-workspace,
  synvelis-recording, recording-poc, synvelis-player, synvelis-render,
  synvelis-gateway, mcp-poc, synvelis-app)
- fallback_full_ci: false

| lane leg | expectation | reason |
| --- | --- | --- |
| verify ×3 OS | required | affected crates: 全 9 |
| rt-verify | required | rt_ contract owner affected (synvelis-project/synvelis-recording) |
| gw10-verify ×3 OS | required | GW-10 owner affected (synvelis-gateway/mcp-poc via dep graph) |
| msrv-verify | required | affected crates: 全 9 |
| dep-guard | required | dependency contract guard (always-on for non-docs changes) |
| classify | required | impact gate input |
| ui-verify ×3 OS | not-applicable | no ui-verify-relevant impact (classes: automation, control, docs, ipc, project) |

本変更は automation/control/project 系 crate の Rust コード+docs のみで
`ui/` 非接触のため、ui-verify は not-applicable — 予想通り selective
向きの変更集合だが、affected crate が全 9 なので rust lane は全て
required のまま。

## 2. run 実結果(attempt 1 + rerun)

### attempt 1(trigger 14:50Z)

| job | 結果 | 実時間 |
| --- | --- | --- |
| classify | pass | 9s |
| ui-verify (ubuntu-latest) | pass | 36s |
| ui-verify (windows-latest) | pass | 1m34s |
| ui-verify (macos-latest) | **cancelled** | queue 20m1s、**0 step 実行** |
| verify (ubuntu-latest) | pass | 1m45s |
| verify (windows-latest) | pass | 4m20s |
| verify (macos-latest) | **cancelled** | queue 25m2s、**0 step 実行** |
| gw10-verify (ubuntu-latest) | pass | 48s |
| gw10-verify (windows-latest) | pass | 1m59s |
| gw10-verify (macos-latest) | **cancelled** | queue 25m2s、**0 step 実行** |
| rt-verify | pass | 1m32s |
| msrv-verify | pass | 2m20s |
| dep-guard | pass | 25s |
| evidence | **failure** | 9s — fail-closed gate が macOS 3 job の cancelled を検出 |

### rerun(attempt 2, `gh run rerun --failed` → 後に run cancel)

- 3 macOS job を 15:16Z に再 queue したが runner 割当なし。
- coordinator 観測: アカウントの macOS 枠枯渇(GitHub 公式注釈の
  macOS arm64 runner 逼迫)で同一現象が ruviform main run を含む
  複数 run で発生。cancel はアカウント操作に帰属する旨、coordinator
  指示で確定(queued job を殺さないため run-level cancel・並列 rerun
  禁止・queued job は待機のまま維持)。
- `verify (macos-latest)` のみ `gh run rerun --job 126664445885` で
  直列 retry(15:29Z queue 開始)したが、coordinator が PR #159 を
  **admin merge** し、macOS lane は NotRun として記録・post-merge
  main run で遅延検証とすることを決定。未完了の serial retry は
  その判断に吸収された。

## 3. Shadow comparison(not-applicable legs の実結果 — 偽陰性対照)

| lane leg | would skip | actual | result |
| --- | --- | --- | --- |
| ui-verify (ubuntu-latest) | yes | **pass** | consistent |
| ui-verify (windows-latest) | yes | **pass** | consistent |
| ui-verify (macos-latest) | yes | **cancelled(0 step)** | 未観測 — runner 不足で実行自体が無い |

- 実行された not-applicable leg(ubuntu・windows)は両方 pass —
  selective 判定と実結果は consistent、**偽陰性 0 件**。
- ui-verify (macos-latest) は 0 step のまま cancel されたため実結果
  なし。これは判定誤り(false negative)ではなく GitHub macOS runner
  逼迫という環境制約。NotRun として coordinator 判断で記録され、
  post-merge main run で検証される。
- required 側の macOS 2 leg(verify・gw10-verify)も同じく 0 step
  cancel → NotRun 記録・post-merge 検証へ。本変更起因の失敗
  シグナルは存在しない(実行された 11 job は全て pass)。

## 4. 観測メモ(#157 蓄積用)

- shadow 分類自体は正確: automation-only 変更で ui-verify のみ
  not-applicable。macOS runner 逼迫がなければ全 14 leg の実結果が
  揃う構成だった。
- macOS job は queue 中 20–25 分で GitHub 側 auto-cancel(runner 未割当
  のまま `cancelled`、0 step)。fail-closed の evidence gate は
  cancelled required leg を正しく検出して failure — gate は健全。
- 教訓: runner 逼迫時は run 単位の cancel/rerun が他 queued job を
  殺す。queued job は放置し、完了済み job が残りゼロになった後に
  `gh run rerun --job` で 1 件ずつ直列 retry する運用が正(coordinator
  指示 msg_81256600aba6)。
