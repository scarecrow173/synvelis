---
id: "PLAN-Tim04Rcp01StateProjection01"
type: "plan"
status: "approved"
title: "TIM-04 RCP-01 persisted plugin-state projection plan"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
---

# TIM-04 RCP-01 persisted plugin-state projection plan

1. 既存 RCP-01 resolver、RCP-02 lookup、TIM-04 manifest/store contract を診断し、projection の data shape を確定する。
2. spec/design/test-spec を作成し、scope、candidate 判定、fail-closed oracle、canonical compatibility を固定する。
3. `synvelis-render` に richer probe outcome、persisted projection、projection error を実装する。
4. projection matrix、recipe resolution、artifact lookup eligibility の縦断 test を追加する。
5. `evidence/sv128/` に machine-readable rows と日本語 summary を残す。
6. 正規 mise lane を実行し、最新 main を取り込んで PR を作成する。

## 検証計画

正規入口は `mise run fmt-check`、`mise run check synvelis-render`、`mise run lint synvelis-render`、`mise run test synvelis-render`、`mise run check synvelis-workspace`、`mise run test-rt`、`mise run check-headless-deps`、`mise run check-ruviform-rev` とする。cargo 直接実行は evidence 生成の補助証拠と明記する。
