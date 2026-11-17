# sv517 — render/branch pipeline 本番配線 evidence

Issue: `scarecrow173/synvelis#517` / branch `feature/517-render-branch-wiring`
OS: Windows (mise managed toolchain)。cargo 直接実行は補助証拠、
canonical = mise。

## Lane 別 Pass/NotRun

| Lane | 判定 | 根拠 |
| --- | --- | --- |
| OS-L1 headless/fixture | **Pass** | 下記 mise 検証コマンド全 pass |
| GUI-L2 | **NotRun** | render UI surface は sv#295 の責務（本 slice は gateway 登録まで） |
| EXT-L3 実 render/RT | **NotRun** | 実 render pipeline・実 RT 計測は sv#267/sv#288/sv#296/sv#114 の別 gate |
| 実 CLI/MCP/GUI E2E | **NotRun** | headless `UnavailableRenderDispatch` wiring のみ — backend 未接続を正直に typed rejection |

## 実行した検証（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt` + `mise run fmt-check` | Pass |
| `mise run check synvelis-project` | Pass |
| `mise run lint synvelis-project` | Pass |
| `mise run test synvelis-project` | Pass（全 suite — sv517_render_wiring 6/6 含む、vcs05/vcs06/cb01-11 回帰なし） |
| `mise run check synvelis-gateway` | Pass |
| `mise run lint synvelis-gateway` | Pass |
| `mise run test synvelis-gateway` | Pass（全 suite — sv517_render_actions 6/6、cb10_variant_actions/commit_gate/job 回帰なし） |
| `mise run check synvelis-workspace` | Pass |
| `mise run lint synvelis-workspace` | Pass |
| `mise run test synvelis-workspace` | Pass（51 test-result ok、rig `render: None` literal 修復含む） |
| `mise run check synvelis-cli` | Pass |
| `mise run lint synvelis-cli` | Pass |
| `mise run test synvelis-cli` | Pass |
| `mise run check mcp-poc` | Pass |
| `mise run lint mcp-poc` | Pass |
| `mise run test mcp-poc` | Pass |
| `mise exec -- prek run --all-files` | Pass |

補助証拠（canonical = mise）: `cargo test -p synvelis-project
--test sv517_render_wiring` 6/6、`cargo test -p synvelis-gateway
--test sv517_render_actions` 6/6（mise test 内でも同結果を確認）。

## 新規 test pin

- `crates/synvelis-project/tests/sv517_render_wiring.rs`（6 件）:
  store restart persistence・digest 検証・corruption fail-closed、
  journaled publish typed Pending/Failed + restart-visible ledger、
  restore fork-copy 成功 + unavailable 正直 verdict。
- `crates/synvelis-gateway/tests/sv517_render_actions.rs`（6 件）:
  9 capability 登録 parity、dispatch parity（project 非変更 +
  provenance）、Pending/Unavailable/port 不在の typed
  non-fabrication、capability floor、idempotent replay。

## 範囲拡張の記録

- `crates/synvelis-workspace/tests/rig/mod.rs`: `GateDeps` への
  `render` field 追加に伴う `render: None` literal 修復 1 行。
  coordinator 承認済み（msg_1cf92f453484）。

## 未検証・残件

- 実 render backend への `RenderDispatch` 実装接続（synvelis-render
  orchestrator — sv#518 領域）。
- runtime-publish journal の engine/session 消費
  （SessionRoutingEngine — sv#514 領域、契約照合のみ実施）。
- `restore_as_copy` の production 呼出配線（現状 restore engine は
  test 経路のみ — `branch_content` port は engine API として提供済み）。
- docs index（`docs/**/README.md`）は Coordinator が統合時に再生成。
