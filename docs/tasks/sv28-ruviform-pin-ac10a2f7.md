---
id: "TASK-zNmgZzwxOh9fJPWJTLu5Un"
type: "task"
status: "done"
title: "SV#28 Ruviform pin を ac10a2f7 へ更新し検証を完了する"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "evidence/sv28/ruviform-ac10a2f7-diff-review.md"
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/impl/ir/sv-28-ruviform-pin-ac10a2f7.md"
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
    - "docs/plans/sv28-ruviform-pin-ac10a2f7.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv28-ruviform-pin-ac10a2f7.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/plans/sv28-ruviform-pin-ac10a2f7.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-ac10a2f7.md"
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
    - "evidence/sv28/ruviform-ac10a2f7-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin を ac10a2f7 へ更新し検証を完了する

## Work

- [x] branch `chore/28-ruviform-pin-ac10a2f7` を `origin/main`
      （`d54d7ae`）から作成
- [x] ruviform `main` HEAD を `ac10a2f7761c08db73aebd861c0a5c1f53f269b7`
      に再解決確認
- [x] `cargo update -p` で 27 package を scoped 更新し
      `Cargo.lock` 差分を検査（source fragment 54 行のみ）
- [x] `mise run sync-ui-deps` で `ui/vendor/ruviform` 再生成
- [x] `node tools/check-ruviform-rev.mjs --require-vendor` で
      REV match 確認
- [x] upstream `git log`/`git diff` 精査と evidence メモ作成
      （`evidence/sv28/ruviform-ac10a2f7-diff-review.md`）
- [x] canonical 検証 lane 実行（fmt-check・check×14・lint×14・
      test×14・test-rt・check-headless-deps・ui・app-webview・
      app-native・app-gpui・recording-real-input lint・prek）
- [x] 7-doc chain + index 再生成
- [x] review-protocol・commit・push・PR（`Refs #28`）・
      Issue #28 コメント

## Classification

pin 更新 slice（依存追従）。製品コード変更なし。

## Upstream Documents

- spec/design/plan/test-spec/discovery の各 `sv28-ruviform-pin-ac10a2f7.md`。

## Dependencies

- ruviform `main` HEAD が dispatch 時点 `ac10a2f7` と一致する
  こと（確認済み・前進なし）。

## Affected Files

- `Cargo.lock`（source fragment 更新）
- `ui/vendor/ruviform`（生成物・非追跡）
- `evidence/sv28/ruviform-ac10a2f7-diff-review.md`（新規）
- `docs/{discovery,specs,designs,plans,test-specs,tasks,impl/ir}/`
  の `sv28-ruviform-pin-ac10a2f7.md` / `sv-28-ruviform-pin-ac10a2f7.md`
- `docs/*/README.md`（index 再生成）

## Done When

- `check-ruviform-rev --require-vendor` が REV match。
- canonical lane が exit 0（実機 lane は NotRun 記録）。
- 7-doc chain + index が揃い、PR が `Refs #28` で作成される。

## Verification

`docs/test-specs/sv28-ruviform-pin-ac10a2f7.md` の Rules R1〜R7
に従う。結果は `docs/impl/ir/sv-28-ruviform-pin-ac10a2f7.md`
の Validation 節に記録。
