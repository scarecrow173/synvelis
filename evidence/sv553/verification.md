# sv553 — variant intent_facts inner op facts 伝播 evidence

Issue: `scarecrow173/synvelis#553` / branch
`bugfix/553-intent-facts-propagation`（base: `origin/main`
`5b9cae01`、前提 PR `synvelis#551`/`synvelis#549` merge
済み）
OS: Windows（mise managed toolchain）。cargo 直接実行は
補助証拠、canonical = mise。

## Lane 別 Pass/NotRun

| Lane | 判定 | 根拠 |
| --- | --- | --- |
| OS-L1 headless/fixture | **Pass** | 下記 mise 検証コマンド全 pass。`cb06_strip_gateway` 8 件が variant-edit 経路の inner facts 伝播（send.remove cascade fail-closed deny）を headless で実測 |
| GUI-L2 | **NotRun** | GUI surface は対象外（headless gateway 契約のみ） |
| EXT-L3 実 render/RT・実 plugin | **NotRun** | 実機/実 PCM/実 RT は外部 gate（Issue が明示的に対象外） |
| 実 CLI/MCP/GUI E2E | **NotRun** | host 側 E2E は本 slice の検証範囲外 |
| MSRV lane | **NotRun** | ユーザー指定により実行しない |

## 実行した検証（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `mise run check synvelis-gateway` | Pass |
| `mise run lint synvelis-gateway` | Pass |
| `mise run test synvelis-gateway` | Pass（30 integration test binaries + lib/bin unittests + doc-tests、0 failed。`cb06_strip_gateway` 8 件 — 新規 `cb06_strip_553_variant_edit_send_remove_unobservable_cascade_denied`・top-level pin `cb06_strip_550_send_remove_unobservable_cascade_denied`・positive variant-edit 群を含む。`authorization.rs` の `sec14_unverifiable_effects_deny` も Pass） |
| `mise run check synvelis-project` | Pass |
| `mise run lint synvelis-project` | Pass |
| `mise run test synvelis-project` | Pass（55 test result 全 ok・0 failed — 本修正は project 非接触だが Issue 検証方法の指定どおり下流整合を実測） |
| `mise run fmt-check` | Pass |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI framework deps 無し） |
| `mise exec -- prek run --all-files` | Pass |

## 補助証拠（canonical = mise）

- red 確認: 修正前コードで新規 test を実行し、
  `cb06_strip_553_variant_edit_send_remove_
  unobservable_cascade_denied` が `Completed` 受理で
  panic（inner `unsupported` が `declared_facts` で
  破棄されていたことを実測）。
- green 確認: 修正後に `cb06_strip_gateway` 全 8 件
  Pass（`cargo test -p synvelis-gateway --test
  cb06_strip_gateway` — 補助証拠）。

## 実測した契約

- `cb06_strip_553_variant_edit_send_remove_
  unobservable_cascade_denied`: send-owned `Macro`
  source を持つ send（head rev 6）に対し、widest
  honest mask（`sends` 宣言済み `strip-fields` scope）
  で fork → candidate `send.remove` edit が
  `PreparationRejected`（reason に "unverifiable"）。
  branch content revision は fork 直後の 1 のまま
  — deny は prepare 段で完結し何も mutate しない。
  mask gating ではなく `declared_facts` の inner
  facts 伝播が deny 原因であることを構造的に分離。
- `cb06_strip_550_send_remove_unobservable_cascade_
  denied`: top-level prepare の同一 deny pin が
  引き続き Pass — `PreparedMutation::Project` arm
  は非接触で回帰なし。
- positive variant-edit 群（`cb06_strip_fields_
  mask_gated_edit_wire_lowering`・`cb06_strip_fields_
  insert_ops_wire_lowering`・`cb06_strip_550_send_ops_
  wire_lowering`・lifecycle）: `unsupported` が空の
  inner op の受理・commit は不変。inner
  `preconditions` の伝播は changeset record 面の
  追加であり gate を変えない。

## 変更サマリ

- `crates/synvelis-gateway/src/variant.rs`:
  `declared_facts` に `preconditions`/`unsupported`
  accumulator を追加し、`VariantIntent::Edit` arm の
  `BranchEditOp::Project` inner `OpFacts` を verbatim
  伝播。doc comment に契約を追記。
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`:
  modulated send fixture の `seed_strip_send_
  modulated_store`/`boot_strip_send_modulated`
  helper と variant-edit deny test を追加。
- docs: issue-553 doc chain 8 type + cb-06 doc 3 件
  追補。
- `crates/synvelis-project/**` は非接触（`OpFacts`
  の send 意味論は synvelis#550 で確定済み・
  読み取りのみ）。

## 範囲外の記録（未対応の作業）

- `VariantIntent::Adopt` 経路の同等 gap: adopt arm
  は inner facts を持たず、post-simulation diff が
  send-owned `ModulationSource`/`Assignment` を
  観測不能と見られるため、adopt 経路に同等の
  不可視削除 gap が残る可能性がある（未検証・
  explore 時点で uncertain）。本 slice では扱わず
  `docs/impl/exp/issue-553-intent-facts.md` E4 に
  記録 — Issue 化の判断は Coordinator。
- `evidence/sv283/` の CB-06 tracker ledger への
  追記は lease 外のため未実施 — 必要なら Coordinator
  が統合時に対応。
- docs index（`docs/**/README.md`）は未同期 —
  Coordinator が統合時に再生成。
