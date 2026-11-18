# sv550 — strip routing ops・scope entity removal evidence

Issue: `scarecrow173/synvelis#550` / branch
`feature/550-strip-routing-ops`（base: `origin/main`
`4e0e8a53`、前提 PR `synvelis#549` merge 済み）
OS: Windows（mise managed toolchain）。cargo 直接実行は
補助証拠、canonical = mise。

## Lane 別 Pass/NotRun

| Lane | 判定 | 根拠 |
| --- | --- | --- |
| OS-L1 headless/fixture | **Pass** | 下記 mise 検証コマンド全 pass。`cb06_strip_fields` 24 件・`cb06_strip_gateway` 6 件が strip routing ops・entity removal・send wire lowering を headless で実測 |
| GUI-L2 | **NotRun** | GUI surface は対象外（headless gateway 契約のみ） |
| EXT-L3 実 render/RT・実 plugin | **NotRun** | 実機/実 PCM/実 RT は外部 gate（Issue が明示的に対象外） |
| 実 CLI/MCP/GUI E2E | **NotRun** | host 側 E2E は本 slice の検証範囲外 |
| MSRV lane | **NotRun** | ユーザー指定により実行しない |

## 実行した検証（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（`cargo fmt --all` 適用後） |
| `mise run check synvelis-gateway` | Pass |
| `mise run check synvelis-project` | Pass |
| `mise run lint synvelis-gateway` | Pass（`match_same_arms` 修正後） |
| `mise run lint synvelis-project` | Pass（`match_wildcard_for_single_variants`・`too_many_lines` 3 件を helper 抽出/allow で解消後） |
| `mise run test synvelis-gateway` | Pass（88 test binaries・0 failed。`cb06_strip_gateway` 6 件 — lifecycle・boundary/capabilities・mask-gated edit・insert ops・send wire lowering・send mask/surface negatives を含む） |
| `mise run test synvelis-project` | Pass（88 test binaries・0 failed。`cb06_strip_fields` 24 件 — #550 節 6 件を含む） |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI framework deps 無し） |
| `mise exec -- prek run --all-files` | Pass |

## 補助証拠（canonical = mise）

- `cb_strip_550_sends_ops_mask_gated`: `{Sends}` mask で
  send tap・connection enable/channel-map・send-owned
  `SendLevel` target の作成/bind/base・第 2 send 作成・
  send 削除が candidate に commit、live bitwise 不変。
- `cb_strip_550_route_and_bus_ops_per_field`:
  `{MainRoute}`/`{Inserts,Sidechain}`/bus-scope `{Bus}`
  で各 family の source-side op を別 fixture で受理、
  未宣言 family は `Unsupported`。
- `cb_strip_550_routing_confinement_and_mask_negatives`:
  `{Fader}` で routing op `Unsupported`、foreign-source
  send/connection・foreign bus configure が
  `ScopeViolation`。
- `cb_strip_550_send_removal_closure_and_adopt`: send
  削除が send・edge・`SendLevel` target・automation
  lane を cascade 削除し、adopt で live から除去。
  candidate-local target は live に漏れず、foreign
  sidechain feed は byte-identical。
- `cb_strip_550_boundary_dependencies_mask_relative`:
  `{Fader}` で main/send edge が boundary、宣言 family
  の edge は scope 所有に移行。
- `cb06_strip_550_send_ops_wire_lowering`: send 4 op
  （create/remove/set-tap/set-gain-target）が candidate
  edit として commit、live の tap・gain binding・send
  数は不変。
- `cb06_strip_550_send_ops_mask_and_surface_negatives`:
  `sends` 未宣言 mask で send op が `variant-unsupported`、
  `synvelis.connection.*`/`synvelis.bus.*` は
  `UnknownCapability`（deferred surface）。

## 発見・修正した defect

- `plan_adopt_writes` の `Removed` write が
  `payload: Null` を emit し、`preflight_ports` が
  `RoutingConnection`/`SendConfig` write の endpoint を
  parse するため routing entity の adopt が
  `InvalidPort` で失敗 → `variant/engine.rs` で Removed
  write に削除 entity の serialization を載せる修正
  （`remove_entity` は payload 不読・`Added`/`Modified`
  検証は不変）。

## review 指摘対応（2 reviewer + validation）

- **gain_target confinement（BLOCKER 修正）**:
  `AddSend`/`SetSendGainTarget` の `gain_target` が
  scope 未検査で、apply の re-own により foreign
  `SendLevel` target の採用が adopt 時に live 所有権を
  奪取し得た → adapter で `require_scope
  (ParameterTarget)` を追加。negative test で
  key-strip send の unbound target 拒否を実測
  （`ScopeViolation`）。
- **send gain rebind（pre-existing bug・封じ込め）**:
  `SetSendGainTarget` Some→Some は apply が旧 target
  の `SendLevel` seal を解放しないため常に失敗
  （`ops.rs`、本 diff 以前から存在・非 lease）。
  adapter で別 target への再 bind を `Unsupported`
  で fail-closed。正規修正は domain 設計を要し別
  Issue 候補。`None`→`Some`/`Some`→`None`/同一 target
  は従来通り。
- **send.remove cascade under-report（BLOCKER 修正）**:
  send-owned `ModulationSource`/`Assignment` は
  `ResourceKind` 非対象で delete が不可視 →
  `op_facts` で pinned project を検査し存在時のみ
  `unsupported`（SEC-14 fail-closed）。top-level
  prepare で "unverifiable effects" deny を実測
  （`cb06_strip_550_send_remove_unobservable_
  cascade_denied`）。**残存 gap**: `variant.rs` の
  `intent_facts` が inner op の `unsupported`/
  `preconditions` を捨てるため variant-edit 経路では
  伝播しない — 全 inner op 共通の pre-existing 系統的
  gap（非 lease）で別 Issue 候補。
- **send.create parent 不整合（NOTE 修正）**:
  contract parent `Strip`→`Project`（Send attrs
  owner=Project と一致、`track.create` と同 posture）。
  lowering の `parent_strip` 撤去。
- **capabilities() doc（NOTE 修正）**: mask-relative
  の誠実な記述へ。

## 変更サマリ

- project: `StripField` 4 variant・`scope_objects` の
  routing 所有導出・adapter の routing op 受理と
  confinement・scope-entity removal・`apply/entity.rs`
  removal arms・capabilities/boundary の mask-relative 化。
- gateway: `synvelis.send.*` 4 capability（op 表・
  lowering・op_facts）。connection/bus/port は
  `ResourceKind` 非存在のため wire deferred。
- test: `cb06_strip_fields.rs` #550 節 6 件・
  `cb06_strip_gateway.rs` send fixture + 2 件・
  `tests/rig/mod.rs` `tagged()` 全 kind 対応。
- docs: issue-550 doc chain 8 type + cb-06 doc 3 件
  追補 + `evidence/sv283/` 追記 + IR/EXP へ review
  対応節を追記。
- review 後再検証: `cargo fmt --all -- --check`・
  `cargo clippy -p synvelis-project -p synvelis-gateway
  --all-targets`・両 crate 全 test suite が pass
  （結果は本 file 更新時点の実行値）。
