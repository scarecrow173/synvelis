# sv557 — VariantIntent::Adopt declared_facts 監査 verification

Issue: `scarecrow173/synvelis#557` / branch
`feature/557-adopt-facts-audit`（base: `origin/main`
`bc90f0e0` — `git fetch origin main && git merge
origin/main` で Already up to date を確認後に実施）
OS: Windows（mise managed toolchain）。cargo 直接実行は
補助証拠、canonical = mise。

## Lane 別 Pass/NotRun

| Lane | 判定 | 根拠 |
| --- | --- | --- |
| OS-L1 headless/fixture | **Pass** | 下記 mise 検証コマンド全 pass。`cb06_strip_gateway` 9 件が adopt 経路の受理契約（send.remove candidate → adopt teardown 物質化）と既存 sv#553/sv#550 pin の非回帰を headless で実測 |
| GUI-L2 | **NotRun** | GUI surface は対象外（headless gateway 契約のみ） |
| EXT-L3 実 render/RT・実 plugin | **NotRun** | 実機/実 PCM/実 RT は外部 gate（Issue が明示的に対象外） |
| 実 CLI/MCP/GUI E2E | **NotRun** | host 側 E2E は本 slice の検証範囲外 |
| MSRV lane | **NotRun** | ユーザー指定により実行しない |

## 実行した検証（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `mise run check synvelis-gateway` | Pass |
| `mise run lint synvelis-gateway` | Pass |
| `mise run test synvelis-gateway` | Pass（30 integration test binaries + lib/bin unittests + doc-tests、0 failed。`cb06_strip_gateway` 9 件 — 新規 `cb06_strip_557_adopt_send_remove_candidate_materializes_teardown`・既存 pin `cb06_strip_553_variant_edit_send_remove_unobservable_cascade_denied`・`cb06_strip_550_send_remove_unobservable_cascade_denied` を含む全件 Pass） |
| `mise run check synvelis-project` | Pass |
| `mise run lint synvelis-project` | Pass |
| `mise run test synvelis-project` | Pass（全 suite green・0 failed — 本 slice は project 非接触だが Issue 検証方法の指定どおり監査対象 domain 層の下流整合を実測） |
| `mise run fmt-check` | Pass |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI framework deps 無し） |
| `mise exec -- prek run --all-files` | Pass（全 hook Passed） |

## 補助証拠（canonical = mise）

- 対象 test 単独実行:
  `mise run test synvelis-gateway --test
  cb06_strip_gateway cb06_strip_557`（underlying:
  `cargo test --locked -p synvelis-gateway --test
  cb06_strip_gateway cb06_strip_557`）— 1 test
  `cb06_strip_557_adopt_send_remove_candidate_
  materializes_teardown` Pass、8 filtered out。
- `cb06_strip_gateway` 全 9 件の内訳（`mise run test
  synvelis-gateway` 出力より）:
  `cb06_strip_fields_scope_boundary_and_capabilities`・
  `cb06_strip_550_send_remove_unobservable_cascade_
  denied`・`cb06_strip_553_variant_edit_send_remove_
  unobservable_cascade_denied`・
  `cb06_strip_550_send_ops_mask_and_surface_negatives`・
  `cb06_strip_fields_insert_ops_wire_lowering`・
  `cb06_strip_fields_gateway_commit_gate_lifecycle`・
  `cb06_strip_557_adopt_send_remove_candidate_
  materializes_teardown`・
  `cb06_strip_fields_mask_gated_edit_wire_lowering`・
  `cb06_strip_550_send_ops_wire_lowering` — 全 Pass。

## 実測した契約

- `cb06_strip_557_adopt_send_remove_candidate_
  materializes_teardown`: fork(`strip_scope()`) →
  candidate `send.remove` edit commit（modulation
  entities を持たない send — post-sv#553 で合法な
  唯一の wire 経路）→ live は adopt まで send 保持 →
  human approval + agent apply で adopt
  `GatewayOutcome::Accepted` + commit 成功 →
  provenance `branch == candidate`・`outcome ==
  "applied"`・`objects == 3`。live から send config・
  `Send` kind `RoutingConnection` edge・send-owned
  `SendLevel` `ParameterTarget` が除去され、adopted
  branch が candidate に更新・旧 branch freeze。
  post-simulation `derive_effects` +
  `authorize_effects` が実 diff を認可する設計を
  固定。
- `cb06_strip_553_variant_edit_send_remove_
  unobservable_cascade_denied`・`cb06_strip_550_
  send_remove_unobservable_cascade_denied`: 既存 pin
  が引き続き Pass — Edit/top-level 経路の非回帰を
  実測（AC3）。

## NotRun / Unsupported

- GUI-L2・EXT-L3（実 PCM/plugin/RT）・実 CLI/MCP/GUI
  E2E・MSRV・3OS 横断・full lane: POLICIES.md §5 に
  より日常必須検証に含めない（Daily Full CI・手動
  Full CI に委譲）。実機/RT/GUI は別 gate 責務。
- pre-553 stored candidate payload の adopt 再現:
  現行 wire で当該 payload を生成不能かつ payload
  捏造が必要なため本 slice では実施せず、hardening
  は `synvelis#559` に委譲（判断根拠は `audit.md`
  §4・doc chain・PR 本文）。
