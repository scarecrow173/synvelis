---
id: "TASK-Tim04PermissionMatrix01"
type: "task"
status: "approved"
title: "TIM-04 permission fault matrix tasks"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
---

# TIM-04 permission fault matrix tasks

## 完了

- [x] `PluginStateStoreOperation` / `PluginStateStorePublishEffect` / typed `PermissionDenied` / `ReadbackFailed` を導入した。
- [x] immutable manifest publish と HEAD update を区別する store contract を追加した。
- [x] project unit test に deterministic permission matrix を追加した。
- [x] real ABI L1 `test_stateful` capture bytes を使う VST3/CLAP evidence matrix を追加した。
- [x] plugin-originated PermissionDenied capability probe を `Unsupported` evidence として追加した。

## 完了処理

- [x] 全必須 verification lane を実行し、結果を `evidence/sv128/tim04-permission-verification.md` に記録した。
- [ ] push、PR 作成。

## 後続 slice / gate

- [ ] plugin-originated PermissionDenied を実測 Pass にする ruviform fixture/HostError capability。
- [ ] Offline render 実 plugin 組込みと `TIM04-RCP-01` end-to-end。
- [ ] typed runtime process oracle、registry/discovery、GUI/Native Editor、AU/市販 plugin、AudioLens、3OS final acceptance。
- [ ] docs index 整備（Coordinator 統合時）。
