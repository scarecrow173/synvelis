---
id: "PLAN-1LEHQHMRbmxKzWPfpYXCvE"
type: "plan"
status: "completed"
title: "SV#28 Ruviform pin 更新 実装計画 — abb9f372 → cbcc8343"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/impl/ir/sv-28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "evidence/sv28/ruviform-cbcc8343-diff-review.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/designs/sv28-ruviform-pin-cbcc8343.md"
  implemented-by:
    - "docs/tasks/sv28-ruviform-pin-cbcc8343.md"
  depends-on:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/designs/sv28-ruviform-pin-cbcc8343.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv28-ruviform-pin-ac10a2f7.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/designs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/discovery/sv28-ruviform-pin-cbcc8343.md"
  derived-by:
    - "docs/test-specs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/tasks/sv28-ruviform-pin-cbcc8343.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-cbcc8343.md"
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
    - "mise.toml"
    - "evidence/sv28/ruviform-cbcc8343-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 実装計画

## Goal

`Cargo.lock` の 28 `ruviform-*` package source を
`cbcc83437616fdd7418da37183c6be66b6cfe225` へ更新し、
`ui/vendor/ruviform` を再生成し、upstream 差分の consumer
影響を評価した上で canonical 検証 lane を完了する。

## Scope

- lockfile 更新 + vendor 再生成 + revision gate 検証。
- upstream diff 精査（evidence メモ）。
- canonical 検証 lane の実行と記録。
- 7-doc chain + PR/Issue #28 連携（docs index 再生成は
  Coordinator 委譲）。

## Upstream Documents

- spec: `docs/specs/sv28-ruviform-pin-cbcc8343.md`
- design: `docs/designs/sv28-ruviform-pin-cbcc8343.md`
- discovery: `docs/discovery/sv28-ruviform-pin-cbcc8343.md`
- 差分精査メモ: `evidence/sv28/ruviform-cbcc8343-diff-review.md`

## Design Inputs

- pin 正本は `Cargo.lock` のみ。`Cargo.toml` の git 依存は
  `rev` 未指定。
- vendor は `mise run sync-ui-deps` が生成。手編集禁止。
- 新規 upstream 機能は全て不採用（design doc 判断表）。
- `crates/synvelis-workspace` は HARD BOUNDARY（`scarecrow173/
  synvelis#488` Worker 占有中）— pin 起因の破壊が出ても修正
  しない。

## File Map

| ファイル | 変更 |
| --- | --- |
| `Cargo.lock` | 28 package の source fragment を `cbcc8343…` へ更新 + `ruviform-audio-test` に `serde`/`serde_json` 依存追加 |
| `ui/vendor/ruviform` | `sync-ui-deps` で再生成（gitignore・非追跡） |
| `evidence/sv28/ruviform-cbcc8343-diff-review.md` | 新規。差分精査メモ |
| `docs/{discovery,specs,designs,plans,test-specs,tasks}/sv28-ruviform-pin-cbcc8343.md`（ir は `sv-28-` prefix） | 新規。7-doc chain |
| `crates/**` | 変更なし（消費面の破壊的変更 0 件） |
| `Cargo.toml` | 変更なし |
| `docs/**/README.md` | 変更なし（index 再生成は Coordinator 委譲） |

## Implementation Sequence

1. branch `feat/28-ruviform-cbcc8343` を `origin/main`
   （`f05bdbf`）から作成。
2. ruviform `main` HEAD を再解決 → `cbcc8343` 確認。
3. `cargo update -p` で 28 package を scoped 更新 →
   `Cargo.lock` 差分検査（source fragment + audio-test 依存）。
4. `mise run sync-ui-deps` → `check-ruviform-rev
   --require-vendor` → REV match 確認。
5. upstream `git log`/`git diff` 精査 → evidence メモ作成。
6. canonical 検証 lane 実行（fmt-check → check → lint →
   test → test-rt → headless-deps → ui/app-webview/app-native/
   app-gpui → recording-real-input lint → prek）。
7. 7-doc chain 作成（index 再生成は実施しない）。
8. review-protocol → commit（lockfile / docs 分割）→ push →
   PR 作成（`Refs #28`）→ Issue #28 コメント。

## Dependencies and Risks

- 依存: ruviform `main` HEAD が dispatch 時点 `cbcc8343` から
  前進していないこと（確認済み）。
- リスク: 消費面の見落とし → `check --all-targets` で担保。
  実機 lane 未実施 → `NotRun` として正直に記録。
  #488 由来の既存 fail → 切り分けて記録、修正しない。

## Task Breakdown

単一 slice。詳細は `docs/tasks/sv28-ruviform-pin-cbcc8343.md`。

## Verification

- 機械検証 lane は test-spec doc の Rules/Guarantees に従う。
- 結果は impl/ir doc の Validation 節に実コマンド・exit code・
  環境を記録する。

## Open Questions / Gaps

- なし。

## Self-Review

- spec の R1〜R6・AC1〜AC5 を全てカバー。
- 新規機能の採用判断は design doc に集約。
- 実機 lane は NotRun として明示。
- docs index 再生成の Coordinator 委譲を明記。

## Implementation Handoff

`docs/tasks/sv28-ruviform-pin-cbcc8343.md` に実施単位を記録。

## Review Handoff

review-protocol で spec/design/plan/test-spec/task/impl の
整合と検証証拠を確認してから commit/push/PR へ進む。
