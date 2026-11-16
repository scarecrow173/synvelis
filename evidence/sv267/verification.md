# Verification — synvelis#267 AUTX-08 headless common E2E slice

Date: 2026-11-15（Leg A+B）/ 2026-11-16（Leg C 追記）。
Branch: `scarecrow173/sv267-autx08-e2e`（Leg A+B）・
`feat/267-autx08-live-adapter-leg`（Leg C、base
`0b57299`）。本記録は headless lane のみを対象とし、実 OS /
GUI-L2 / EXT-L3 / live adapter の受入を一切主張しない
（POLICIES.md §2-3、`docs/test-specs/autx-08-common-e2e.md`
Non-goals）。Leg C は production `GatewayStepLane` の
公開 `StepLane` surface（`invoke`/`operation_result`）
parity を headless で固定したものであり、`step_wire`
wire byte 一致・live catalog・dispatch-hop・job lane・
real-OS/EXT-L3 は対象外。

## 検証環境

| 項目 | 値 |
| --- | --- |
| OS | Windows（worktree `G:\orca\workspaces\synvelis\sv267-autx08-e2e`） |
| Toolchain | mise 管理（`mise run` / `mise exec --` 経由） |
| 対象 package | `synvelis-workspace`（test-only 変更＋rig 抽出） |
| Lane | OS-L1 headless のみ |

## 実測結果

| Lane | コマンド | 結果 |
| --- | --- | --- |
| format | `mise run fmt-check` | **Pass**（`cargo fmt --all -- --check` clean） |
| check | `mise run check synvelis-workspace` | **Pass**（`cargo check --locked --all-targets`） |
| lint | `mise run lint synvelis-workspace` | **Pass**（clippy `-D warnings`、初回 2 件を修正後 clean） |
| package test | `mise run test synvelis-workspace` | **Pass** — 全 test binary green。`tests/autx08_e2e.rs` 4/4: `autx08_same_wire_lands_same_outcome_across_surfaces`・`autx08_unavailable_revision_fails_identically_across_surfaces`・`autx08_agent_without_authority_fails_closed_gui_human_permits`・`autx08_idempotent_replay_names_same_operation` |
| headless deps | `mise run check-headless-deps` | **Pass** — `check-headless-deps: no GUI framework deps in 12 headless crates` |
| rt contract | `mise run test-rt` | **Pass**（`cargo test --workspace --locked rt_`、workspace 全体 green） |
| pre-commit | `mise exec -- prek run --all-files` | **Pass**（trailing-whitespace / EOF / yaml / toml / line-ending / BOM / private-key / actionlint） |
| doc index | `doc_maintenance.js index apply` | **Pass** — `docs/impl/ir/README.md`・`docs/test-specs/README.md` materialized（planHash `3b8983b1…45b88`、2 written） |
| impl index | `sync_impl_index.js --kind ir --apply` | **Pass** — `docs/impl/ir` in-sync（243 sources） |
| doc audit | `doc_status.js audit --blocking` | 本 slice の file に finding なし。repo 全体の 707 blocking finding は既存 debt（legacy ID 形式・broken relation）で本変更と無関係 |

## Leg C 実測結果（2026-11-16、branch `feat/267-autx08-live-adapter-leg`）

| Lane | コマンド | 結果 |
| --- | --- | --- |
| format | `mise run fmt-check` | **Pass**（`cargo fmt --all -- --check` clean） |
| check | `mise run check synvelis-workspace` | **Pass**（`cargo check --locked --all-targets`） |
| lint | `mise run lint synvelis-workspace` | **Pass**（clippy `-D warnings` clean） |
| package test | `mise run test synvelis-workspace` | **Pass** — 全 test binary green。`tests/autx08_e2e.rs` 8/8: Leg B 4 test 非回帰 + 新規 `autx08_live_adapter_lands_same_outcome`・`autx08_live_adapter_unavailable_revision_fails_identically`・`autx08_live_adapter_without_authority_fails_closed`・`autx08_live_adapter_duplicate_reconciles_original` |
| targeted test | `cargo test -p synvelis-workspace --locked --test autx08_e2e` | **Pass** — 8 passed; 0 failed（補助証拠、canonical = mise） |
| headless deps | `mise run check-headless-deps` | **Pass** — `check-headless-deps: no GUI framework deps in 12 headless crates`（dev-dep 追加後も GUI/GPU/WebView crate を引き込まない） |
| rt contract | `mise run test-rt` | **Pass**（`cargo test --workspace --locked rt_`、workspace 全体 green） |
| pre-commit | `mise exec -- prek run --all-files` | **Pass** |
| doc audit | `audit_impl_record.js`・`audit_experiment_log.js`・`doc_status.js lint` | 本 slice の新規 doc に blocking finding なし |

`Cargo.lock` 差分は `synvelis-workspace` deps 欄への
`synvelis-automation` 1 行追加のみ（dev-dep 追加に伴う
lockfile 更新 — 共通契約の共有ファイル例外として PR 本文に
明記）。

## lint 修正記録

`mise run lint synvelis-workspace` 初回で `autx08_e2e.rs` に
2 件の clippy error:

1. `needless_pass_by_value`: `gui_leg(consistency: Value)` →
   `&Value` に変更し call site を `&expected(n)` に更新。
2. `doc_markdown`: doc comment の `CallToolResult` を
   `` `CallToolResult` `` に backtick 化。

修正後 lint clean。

## headless Pass の範囲（昇格禁止の明記）

以下は本 slice で **実行していない**（`acceptance-ledger.md`
NotRun/Deferred と同一集合）:

- 実 Windows GUI-L2（実 GPUI window・実 OS 入力・実
  windowing/session lifetime）— #114/#267 受入 lane。
- EXT-L3（実 CLI binary・実 credential store・実 MCP transport
  process・実 provider/plugin/OS API）。
- `synvelis-automation` live adapter からの catalog 実供給
  （#264）・workflow coordinator live run（#265）。
- multi-process durable commit・実 FS fault・実 provider を含む
  E2E。
- CI required lane（`*-msrv` / `*-full` / Feature Matrix /
  3OS 横断）— Daily Full CI 委譲。本 PR の CI 結果は PR 作成後に
  別途記録する。

headless の 4 test が green であることは、3 surface が同一 wire
semantics と同一 `Gateway` pipeline を通ることの **OS-L1 意味
層での証明**であり、実機受入の代替ではない。
