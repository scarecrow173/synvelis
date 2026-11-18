# sv559 — VariantIntent::Adopt unobservable-removal fail-closed verification

Issue: `scarecrow173/synvelis#559` / branch
`bugfix/559-adopt-unobservable-removal`（base: `origin/main`
`848ebb73` — PR #560（synvelis#557 監査）着地後）
OS: Windows（mise managed toolchain）。cargo 直接実行は
補助証拠、canonical = mise。

## Lane 別 Pass/NotRun

| Lane | 判定 | 根拠 |
| --- | --- | --- |
| OS-L1 headless/fixture | **Pass** | 下記 mise 検証コマンド全 pass。domain pin `cb_strip_559_adopt_unobservable_removal_rejected` と gateway pin `cb06_strip_559_adopt_unobservable_removal_denied` が adopt fail-closed 契約を headless で実測。cb06 両 suite で sv#550/#553/#557 既存 pin の非回帰を確認 |
| GUI-L2 | **NotRun** | GUI surface は対象外（headless gateway 契約のみ） |
| EXT-L3 実 render/RT・実 plugin | **NotRun** | 実機/実 PCM/実 RT は外部 gate（Issue が明示的に対象外） |
| 実 CLI/MCP/GUI E2E | **NotRun** | host 側 E2E は本 slice の検証範囲外 |
| MSRV lane | **NotRun** | ユーザー指定により実行しない |

## 実行した検証（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `mise run check synvelis-project` | Pass |
| `mise run lint synvelis-project` | Pass |
| `mise run test synvelis-project` | Pass（unit + 全 integration suite + doc-tests、0 failed。`cb06_strip_fields` 25 件 — 新規 `cb_strip_559_adopt_unobservable_removal_rejected`・既存 `cb_strip_550_send_removal_closure_and_adopt` 等を含む全件 Pass） |
| `mise run check synvelis-gateway` | Pass |
| `mise run lint synvelis-gateway` | Pass |
| `mise run test synvelis-gateway` | Pass（全 integration test binaries + lib/bin unittests + doc-tests、0 failed。`cb06_strip_gateway` 10 件 — 新規 `cb06_strip_559_adopt_unobservable_removal_denied`・既存 `cb06_strip_550_send_remove_unobservable_cascade_denied`・`cb06_strip_553_variant_edit_send_remove_unobservable_cascade_denied`・`cb06_strip_557_adopt_send_remove_candidate_materializes_teardown` を含む全件 Pass） |
| `mise run fmt-check` | Pass（`cargo fmt -p synvelis-project -p synvelis-gateway` 適用後） |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI framework deps 無し） |
| `mise exec -- prek run --all-files` | Pass（全 hook Passed） |

## 補助証拠（canonical = mise）

- domain pin 単独実行:
  `cargo test --locked -p synvelis-project --test
  cb06_strip_fields cb_strip_559` — 1 test
  `cb_strip_559_adopt_unobservable_removal_rejected`
  Pass。
- gateway pin 単独実行:
  `cargo test --locked -p synvelis-gateway --test
  cb06_strip_gateway cb06_strip_559` — 1 test
  `cb06_strip_559_adopt_unobservable_removal_denied`
  Pass。
- 再現（pre-fix）: 修正前コードで同 scenario の test が
  `AdoptOutcome::Applied` で Pass することを確認し
  `evidence/sv559/reproduction.md` に記録。fix 後は同
  scenario が typed `VariantError::UnverifiableRemoval`
  で拒絶されることを pin で固定。

## 実測した契約

- `cb_strip_559_adopt_unobservable_removal_rejected`
  （domain）: send-owned LFO `ModulationSource` +
  `SendLevel` `ParameterTarget` + `ModulationAssignment`
  を持つ fixture で fork(`StripField::Sends`) →
  candidate `RemoveSend` edit（cascade が send・edge・
  level target・modulation source・assignment を削除）
  → `try_adopt` が `Err(VariantError::UnverifiableRemoval)`
  を返し、live の send・modulation closure・
  `live_revision`・`adopted` branch が不変。
- `cb06_strip_559_adopt_unobservable_removal_denied`
  （gateway）: wire-legal `SetSendGainTarget(null)` edit
  で assignment 削除を含む candidate payload を永続化
  → adopt prepare が `PreparationRejected`（reason
  `variant-unverifiable-removal`）を返し live 非破壊。
  proposal/approval/commit には到達しない。
- 非回帰: `cb06_strip_557_adopt_send_remove_candidate_
  materializes_teardown` が引き続き Pass — 観測可能な
  send teardown（send config・`Send` edge・send-owned
  `SendLevel` target・`RoutingConnection` の `Removed`
  write を含む）は adopt で物質化される既存契約を維持
  （`RoutingConnection` は reject 対象外 — sv#550 の
  internal-structure 境界）。

## NotRun / Unsupported

- GUI-L2・EXT-L3（実 PCM/plugin/RT）・実 CLI/MCP/GUI
  E2E・MSRV・3OS 横断・full lane: POLICIES.md §5 に
  より日常必須検証に含めない（Daily Full CI・手動
  Full CI に委譲）。実機/RT/GUI は別 gate 責務。
- `SetSendGainTarget`/`RemoveTrack` 等の live/edit 経路
  `op_facts` unverifiable coverage gap は本 slice 範囲外
  — `synvelis#561` に記録（design residual 節参照）。
- `RoutingConnection` 単独削除・観測不能 kind の
  `Added`/`Modified` write の非対称は design residual
  として記録済み（`synvelis#561` 領域・follow-up）。
