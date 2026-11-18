---
id: "IMPL-034ZEtR2ywHkcfxKNHPPTV"
type: "impl"
status: "completed"
title: "Issue #567 impl IR — async quota pend receipt flake 修正記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/567"
  changes:
    added:
      - type: "file"
        path: "docs/impl/exp/issue-567-quota-pend-receipt.md"
      - type: "file"
        path: "docs/impl/exp/issue-567-quota-pend-receipt-exp.jsonl"
      - type: "dir"
        path: "evidence/sv567/"
    modified:
      - type: "file"
        path: "crates/synvelis-render/src/render_job/scheduler.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/rcp03_render_policy.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-567-quota-pend-receipt.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/567"
    - "evidence/sv567/reproduction.md"
    - "evidence/sv567/verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-567-quota-pend-receipt.md"
    rejected: []
---

# Issue #567 impl IR — async quota pend receipt flake 修正記録

## Summary

`async_pol06_concurrency_quota_pends_then_resumes` の Windows CI
flake（`verify (windows-latest)` で `Pending{QuotaExceeded}` receipt
assert が散発的に失敗）を修正した。根因は scheduler/orchestrator の
欠陥ではなく **test-design race** — B の submit を担う helper
thread の `try_send` 実行タイミングがテスト内のどの同期点からも
制御されておらず、A の terminal 遷移後の drain に滑り込むと
`pending_gate` が active=0 を見て B を即受理し pend receipt が
記録されない。`JobScheduler` に nonblocking `enqueue_intent` を
追加し、test は helper thread を廃して `enqueue ≺ release` の
happens-before を program order で構造保証する形に再構成した。
assertion の意味（B に `Pending{QuotaExceeded}` と `Resumed`、
両 job `Published`、executor calls==2）は一切変更していない。

## Context

- Issue `synvelis#567`（OPEN 維持 — 本 slice では close しない）。
- 関連: `synvelis#518`（async scheduler slice —
  `JobScheduler`/`GateExecutor` の導入元）、`synvelis#292`
  （RCP-03 render policy contract tests）、`ruviform#744`
  （未解決の `JobExecutor` 論点 — 本 slice は `OfflineJob` 経路を
  変更しない）。
- 失敗 signature: `verify (windows-latest)` → `mise run
  test-full` → `rcp03_render_policy` で当該テストのみ
  `quota pend stays receipt-visible under async` で fail。
  CI run `44267395469` @ `b73018bd`・`44227068039` @ `bc90f0e0`
  の 2 連続 run で再現。同期 twin
  `pol06_concurrency_quota_pends_then_resumes`・他 async_pol06
  5 本は同 run で pass。
- 根因判定の詳細と決定的再現手順は
  `evidence/sv567/reproduction.md`、検証記録は
  `evidence/sv567/verification.md` を参照。

## Implementation

### Changes

- `crates/synvelis-render/src/render_job/scheduler.rs`
  - private `enqueue` helper を新設: reply channel を作り
    `try_send` のみ行い `Receiver<T>` を返す（`Full`→
    `QueueFull`、`Disconnected`→`Shutdown` の typed error は
    従来どおり）。既存 `call` は `enqueue(build)?.recv()` に
    委譲する形へ整理し、send+wait の一体実装を 1 箇所に維持。
  - public `enqueue_intent(&self, intent) ->
    Result<Receiver<SubmitOutcome>, SchedulerError>` を追加。
    worker の submit 評価を待たず、command が queue に載った
    時点で返る nonblocking surface。ordering semantics を
    doc comment に明記。
  - `submit_intent` は `enqueue_intent(...).recv()` に書き換え、
    blocking 版の公開契約（戻り値・typed error）は不変。
- `crates/synvelis-render/tests/rcp03_render_policy.rs`
  - `async_pol06_concurrency_quota_pends_then_resumes` の helper
    thread（`std::thread::spawn` + `submit_intent`）を廃止し、
    `scheduler.enqueue_intent(i_b)` で B の intent を enqueue
    してから `entered_rx` 確認・`release_tx.send` を行う形に
    再構成。reply は release 後に `recv_timeout` で回収。
- `docs/impl/exp/issue-567-quota-pend-receipt.md` +
  `issue-567-quota-pend-receipt-exp.jsonl`: 採否記録。
- `evidence/sv567/reproduction.md` / `verification.md`: 証拠。

### Decisions During Implementation

- **所有層の選択**: 不変条件「B の `SchedulerMsg::SubmitIntent`
  は A の slot 解放を引き起こす release より先に queue に入る」
  の破れは scheduler の command enqueue API の表現力不足
  （send 完了を caller が観測できない）に起因する。orchestrator
  側の pend 判定・receipt 記録に欠陥はなく、test 側だけで
  happens-before を作るには「enqueue 完了の観測」が必須のため、
  scheduler 境界に最小の nonblocking enqueue surface を足す
  判断とした。API 追加の justification: 既存 `call` は
  `try_send`+`recv` の一体形で enqueue 完了点が caller に
  見えず、worker が park 中の状況では blocking 版は
  そもそも呼べない（deadlock）。`enqueue_intent` はその
  ギャップを最小形で埋め、blocking `submit_intent` は同一
  enqueue 経路上の薄い wrapper として保持（重複実装なし）。
- **決定性の構造論証**（AC 要求 — timing ではなく構造）:
  (i) `enqueue_intent` は `try_send` 成功時点で command が
  queue に存在することを保証して返るため、test thread の
  program order で `enqueue_intent(i_b)` ≺ `release_tx.send(())`
  の happens-before が成立する。(ii) worker は A の
  `GateExecutor` render 内で park している間 command drain を
  実行できず、`entered_rx` 受信は「A が render 内にいる」ことの
  観測点。release 後も A は 2 件目 demand の reuse + verify
  残り quantum を持ち非 terminal のまま残るため、B の
  `SubmitIntent` を drain する全ての時点で A は slot を保持し、
  `pending_gate` は必ず `active≥1` を見て `Pending
  {QuotaExceeded}` を記録する。A terminal 後にしか drain
  できない状況は構造上存在しない。
- **却下した代替案**: helper thread 残留 + 観測手段追加
  （結局 enqueue 分離が必要で helper の利益なし）、
  `GateExecutor` 内からの enqueue（同じく nonblocking API が
  必要で test thread 直呼びより複雑）、sleep/polling/retry
  による順序付け（AC 禁止）、assertion 弱体化（禁止）。
  詳細は EXP 記録参照。
- **他 async テストの扱い**: `async_pol06_cancel_queued_job_
  is_typed` も helper thread 構造を持つが、A が 16 demand で
  release 後の残り quantum が広く、`b_rx.recv_timeout(2s)` が
  helper へ scheduling 余裕を与えるため観測 flake はない。
  無差別な全書き換えは避け、理論的残存 race として残件に
  記録した（`synvelis#518` 系の将来 flake 時に同パターンで
  対応可能）。

## Related Experiments

### Adopted

- `docs/impl/exp/issue-567-quota-pend-receipt.md`（EXP-3:
  nonblocking `enqueue_intent` + test 再構成）— 決定性を構造で
  保証できる唯一の候補として採用。

### Rejected

- EXP-1: helper thread 残留 + enqueue 観測手段 — send 完了が
  不可視な `call` の構造上、観測には結局 enqueue 分離が必要。
- EXP-2: scheduler defect（receipt 喪失）仮説 — `push_receipt`
  は `orch.submit` 内で reply 前に同期 append、`job_receipts`
  は filter のみで喪失経路なし。決定的再現でも「B が pend を
  踏まなかった」ことを確認。
- EXP-4: `GateExecutor` 内からの enqueue — nonblocking API が
  前提で必要になり、test thread 直呼びより複雑。
- EXP-5: 同型 helper 構造の他テスト同時書き換え — 観測失敗が
  1 本のみで、残りは構造的余裕あり。残件記録に留める。

## Validation

- 修正前再現: helper thread へ 200ms sleep を挿入した決定的
  再現で CI と同一 signature を確認
  （`evidence/sv567/reproduction.md`、補助証拠 =
  `mise exec -- cargo test`）。
- 修正後: `async_pol06` filter 5 テスト pass、flake loop
  42 iteration（3 並列・CPU 競合下）全 pass、
  `mise run check/lint/test synvelis-render`・
  `mise run fmt-check`・`mise run check-headless-deps`・
  `prek run --all-files` — 詳細と exit code は
  `evidence/sv567/verification.md`。
- Windows CI lane（PR `verify (windows-latest)`）は push 後の
  PR CI で確認 — 統合検証は merge 後対象 SHA の CI 証拠
  （Coordinator 確認）。

## Risks

- `enqueue_intent` は public API 追加 — enqueue 済み intent の
  reply を caller が `recv` し損ねると worker は通常どおり
  処理するが outcome が捨てられる。doc comment で ordering
  semantics を明記済み。fire-and-forget 用途を想定しない
  ことを API 形状（`Receiver` 返却）で表現。
- `async_pol06_cancel_queued_job_is_typed` に同型の理論的
  race が残存（観測失敗なし・実測 flake なし）。

## Follow-ups

- 同型 helper 構造を持つ async テスト
  （`async_pol06_cancel_queued_job_is_typed` 等）に観測 flake
  が出た場合は `enqueue_intent` で同パターン適用可能。
- docs index（`docs/impl/{ir,exp}/README.md`）は本 slice では
  未同期 — Coordinator が統合時に `doc-maintenance` で
  再生成する。
