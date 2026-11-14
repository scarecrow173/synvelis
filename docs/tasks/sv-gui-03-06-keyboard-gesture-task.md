---
id: "TASK-6q3mKoDF4oDwOgkoL86SR4"
type: "task"
status: "done"
title: "SV-GUI-03 slice6 Keyboard/Gesture key-assignment contract 文書チェーン"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv-gui-03-06-keyboard-gesture-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv-gui-03-06-keyboard-gesture-plan.md"
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/impl/ir/sv-gui-03-06-keyboard-gesture.md"
    - "docs/impl/exp/sv-gui-03-06-keyboard-gesture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sv-gui-03-06-keyboard-gesture-plan.md"
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice6 Keyboard/Gesture key-assignment contract 文書チェーン Task

## Overview

Issue #112 の docs-only スライスとして、daw-workflow(Issue #8)
が #112 へ委譲したキーボード/ジェスチャ項目を canonical
契約文書チェーン(spec → design → plan → task → test-spec →
impl ir/exp)と `evidence/sv112/` の case ledger・検証記録で
確定する。`crates/` のコード変更・実 OS/実 GUI/実機検証は
対象外。keymap dispatch 実装は実機 PoC へ Deferred。

## 上流文書

- spec: `docs/specs/sv-gui-03-06-keyboard-gesture.md`(approved)
- design: `docs/designs/sv-gui-03-06-keyboard-gesture-design.md`
  (approved) — §3.2 KG-KEY-01〜15・§4.1 行列・§5 安全契約・
  §6 メニュー網羅・§8 KG-D-1〜8 が契約本文
- plan: `docs/plans/sv-gui-03-06-keyboard-gesture-plan.md`
  (approved)

## 作業内容

- [x] spec 作成・review-protocol 合格・approved 化
- [x] design 作成・review-protocol 合格・approved 化
- [x] plan 作成(approved design 前提)・approved 化
- [x] test-spec 作成・内容記入(verifies=spec)
- [x] evidence/sv112/ に case ledger(委譲項目→契約セクション
  →受入状況)と検証記録を作成
- [x] impl ir/exp 作成・内容記入
- [x] doc-maintenance `index plan`→`index apply` で触れた
  dir の index を再生成
- [x] `doc_status lint/audit`・`mise run fmt-check`・
  `git diff --check` で検証
- [x] Conventional Commit → push(自分のブランチのみ)→
  PR 作成(日本語本文、`Refs #112`、Issue は Close しない)

## Verification

```sh
# 1. docs index 再生成(doc-maintenance Skill の手順)
node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/specs
node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/designs
node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/plans
node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/tasks
node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/test-specs
node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/impl
node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply --dir docs/specs
# ... (apply を各 dir で実施)

# 2. doc-status lint/audit
node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/specs
node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/designs
node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/plans
node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/tasks
node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/test-specs
node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/impl
node .agents/skills/doc-status/scripts/doc_status.js audit

# 3. impl IR 監査
node .agents/skills/impl-doc/scripts/audit_impl_record.js --json

# 4. fmt-check(コード変更なし。no-op 確認)
mise run fmt-check

# 5. 空白・範囲チェック
git diff --check
git status --short
```

期待 exit 条件: 全コマンド exit 0。`doc_status lint` は新規
文書由来の blocking finding がないこと(後続工程で解消する
index 未反映の warning は apply 後に解消)。`git status` の
変更が許可範囲 + 生成 index + `evidence/sv112/` のみである
こと。

## Done Criteria

- KG-AC-01〜07 が契約文書・test-spec・evidence 台帳で検証
  可能な形に規定されている。
- `evidence/sv112/` に case ledger と検証記録があり、
  実 GUI key dispatch・実 OS/GUI ゲートが NotRun/Deferred
  と正直に記録されている。
- docs index が doc-maintenance で再生成され、手編集がない。
- `crates/`・`Cargo.toml`/`Cargo.lock`/`mise.toml`/
  `.github/workflows/`・範囲外 docs/evidence に差分がない。
- PR 本文が日本語で、実行した検証・未検証項目・
  review-protocol 結果・未対応の作業を記載し `Refs #112`
  を含む。
