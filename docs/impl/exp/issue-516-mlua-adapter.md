---
id: "IMPEXP-issue-516-mlua-adapter"
type: "impl-exp"
status: "done"
title: "Issue #516 experiment log — 実装中の試行と判断"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/516"
    - "docs/impl/ir/issue-516-mlua-adapter.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-516-mlua-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #516 experiment log

## E1 直接 crate 交換の可否調査

- 試行: `synvelis_script::backend` trait をそのまま
  `ruviform_script` へ差し替えられるか照合。
- 結果: **不可** — trait 署名・error enum・SessionConfig 形状・
  digest 形式が全面的に異なり drop-in は成立しない。
- 判断: 薄い binding（anti-corruption adapter）で synvelis
  公開面を維持しつつ製品 backend を消費する分岐を採用。

## E2 ruviform pin 更新の必要性検証

- 試行: `Cargo.lock` が `cbcc8343` → `84a68f25` へ全 ruviform
  crate 一括更新された差分を「意図しない広範囲変更」と疑い、
  旧 pin に `ruviform-script-luau` が存在するか確認。
- 結果: 旧 pin `cbcc8343` には同 crate が**存在しない**
  （runtime backend `dce45712`/PR #729 以降）。rev 更新は必須。
- 判断: 一括更新は cargo の git source 統一による必然的帰結 —
  記録のみ・差し戻し不可。PR 本文に明記。

## E3 artifact payload Record 必須の分類差

- 試行: 製品契約は artifact payload を `BTreeMap`（Record 必須）
  とし、bare scalar / 非 record は `Internal` で拒否。interim
  は `ScriptValue` 全体を受理していた。
- 結果: `tests/backend.rs` で non-record artifact を返す 2 test が
  `Internal` で失敗 — 製品契約の方が厳格。
- 判断: adapter は Record map を `ScriptValue::Record` で包み戻す
  が、Record 必須の fail-closed は緩めない。test を record 形へ
  更新し、bare scalar → `Internal` を検証する形に変更。
  consumer は常に spec record を期待するため実害なし。

## E4 `BudgetExceeded.kind` の帰属

- 試行: 製品 `ScriptErrorKind::BudgetExceeded` は unit variant で
  次元 payload を持たない。interim の `BudgetKind` をどう維持するか。
- 結果: `budget` の `requested_kinds()` ∩ `enforced` が単一の
  ときのみ `Some(kind)` を正直に返せる。複数併用時は中立 error が
  次元を運ばず帰属不可能。
- 判断: port を `Option<BudgetKind>` へ拡張し、`None` で
  「不明」を正直に返す。consumer は `kind` を直接 match していない
  ため影響なし（lease 内 `backend.rs` のみ変更）。

## E5 clippy 修正

- `open_session` が `too_many_lines`（101/100）→
  `product_artifact_kinds` を抽出。
- `Internal`/`match_same_arms`（future variants と wildcard 重複）→
  wildcard arm へ統合。
- diagnostic severity の `Error`/wildcard 重複 → `Info|Hint`・
  `Warning`・wildcard の 3 arm へ整理。
