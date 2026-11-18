---
id: "EXP-567QuotaPendReceipt01"
type: "experiment"
status: "completed"
title: "Issue #567 impl EXP — async quota pend receipt flake 実験記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/567"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/issue-567-quota-pend-receipt.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-567-quota-pend-receipt.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-561-opfacts-unverifiable-cascade.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-567-quota-pend-receipt.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #567 impl EXP — async quota pend receipt flake 実験記録

## 実験サマリ

Windows CI で散発する
`async_pol06_concurrency_quota_pends_then_resumes` の
`Pending{QuotaExceeded}` receipt assert 失敗について、根因仮説
（test-design race vs scheduler defect）を検証し、修正候補の
採否を記録する。詳細なイベントログは
`issue-567-quota-pend-receipt-exp.jsonl` を参照。

## 根因判定の経緯

1. **失敗 signature の確認**: `verify (windows-latest)` →
   `mise run test-full` → `rcp03_render_policy` で当該テスト
   のみ fail（CI run `44267395469`・`44227068039`）。他
   async_pol06・同期 twin は pass。
2. **機構のコード検証**: `JobScheduler` の worker loop は
   「全 command drain → `pick_candidate` → `drive_step` 1
   quantum」。`submit_intent` = `try_send` + `recv` の一体形で、
   helper thread の `try_send` 実行タイミングはテストの同期点
   から制御不能。`GateExecutor` は worker thread を park する
   が、park 前後の drain 順序は OS scheduling 依存。
3. **receipt 喪失仮説の棄却**: `ReceiptKind::Pending` は
   `orch.submit` 内で reply 送信前に `push_receipt` で同期
   append。`job_receipts` は key 一致 filter のみ。記録済み
   receipt が失われる経路は存在しない。
4. **決定的再現**: helper thread の `submit_intent` 前に
   200ms sleep を挿入 → CI と同一 signature
   （`quota pend stays receipt-visible under async`）を再現。
   「B の submit 評価が A terminal 後にずれ込み pend 判定が
   走らなかった」ことを確認（`evidence/sv567/reproduction.md`）。

## 採否記録（exp.jsonl と対応）

| EXP | 仮説 | 結果 | 要点 |
| --- | --- | --- | --- |
| EXP-1 | helper thread を残し test thread が B の `try_send` 完了を観測する手段を足すべき | rejected | `call` は send+wait の一体形で send 完了は不可視。観測手段の新設は結局 enqueue 分離を要求し helper 残留の利益がない |
| EXP-2 | B は pend したのに receipt が失われた scheduler defect | rejected | `push_receipt` は `orch.submit` 内で reply 前に同期 append、`job_receipts` は filter のみで喪失経路なし。決定的再現で「B が pend を踏まなかった」ことを確認 |
| EXP-3 | `JobScheduler` に nonblocking `enqueue_intent` を追加し test を helper thread 無しで `enqueue ≺ release` の happens-before を構造保証する | **採用** | `enqueue_intent` は `try_send` 成功時点で返り command の queue 存在を保証。worker は A の parked render 中に drain できず、release 後も A は非 terminal のまま残る quantum を持つため、B の submit 評価時点で A は必ず slot を保持 — 決定性が構造で示せる。blocking `submit_intent` は同一路上の薄い wrapper として保持 |
| EXP-4 | `GateExecutor` 内（worker thread 上）から B を enqueue する案 | rejected | worker thread 上では blocking `submit_intent` は reply wait で deadlock するため結局 nonblocking API が必要。test thread からの直呼びの方が単純で happens-before も program order で自明 |
| EXP-5 | `async_pol06_cancel_queued_job_is_typed` 等の同型 helper 構造も同時に `enqueue_intent` 化すべき | rejected | 観測された失敗は 1 本のみ。cancel テストは A が 16 demand で残り quantum が広く `b_rx.recv_timeout(2s)` が helper へ scheduling 余裕を与える構造で実測 flake なし。無差別な全書き換えは避け残件として記録 |

## 関連リンク

- IR: `docs/impl/ir/issue-567-quota-pend-receipt.md`
- 検証: `evidence/sv567/verification.md`・
  `evidence/sv567/reproduction.md`
