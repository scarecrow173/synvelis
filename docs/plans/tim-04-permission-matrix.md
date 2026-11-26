---
id: "PLAN-Tim04PermissionMatrix01"
type: "plan"
status: "approved"
title: "TIM-04 permission fault matrix plan"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
---

# TIM-04 permission fault matrix plan

1. 既存 TIM-04 store/session/receipt/real ABI harness を診断し、Permission の責務境界を確定する。
2. `PluginStateStoreError` に構造化された operation/effect 付き permission denial を追加し、staged publish/read error を境界で分類する。
3. project unit test に deterministic matrix を実装し、publish/read/restore denial、Strict/Standard、旧 state 保護、indeterminate+readback denial を検証する。
4. L1 real ABI test に actual RFST bytes 入力の matrix evidence と plugin-originated capability probe を追加する。
5. evidence/sv128 と docs chain を日本語で作成する。
6. canonical mise lane を実行し、最新 main を取り込んで PR を作成する。

## 検証計画

正規入口は `mise run fmt-check`、`mise run check/lint/test synvelis-project`、`mise run check/lint/test synvelis-player`、`mise run test-rt`、`mise run check-headless-deps`、`mise run check-ruviform-rev`。cargo 直接実行は evidence 生成の補助証拠と明記する。
