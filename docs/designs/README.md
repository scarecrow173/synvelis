# Architecture Decision Records

<!-- doc-suite:generated-index -->

Directory: `docs/designs`

| ID | Title | Status | File |
| --- | --- | --- | --- |
| DESIGN-OVERVIEW | Synvelis 設計概要 | draft | [overview.md](./overview.md) |
| DESIGN-pzR2GiO8Eru6Jw2zRK6l2s | ALIB-01 Design — Asset Provider 契約・Local/Project 基準 Provider | approved | [alib-01-asset-provider-contract-design.md](./alib-01-asset-provider-contract-design.md) |
| DESIGN-034AUT01Kp3mWv9zR2sQfL6h | AUT-01 Touch Return・Writer State 境界設計 — session clock + 明示閉鎖 | approved | [aut-01-touch-return-writer-state-design.md](./aut-01-touch-return-writer-state-design.md) |
| DESIGN-034AUT02Qm8wXs5vLn3jKp9r | AUT-02 Region Replace 境界曲線保存 設計 — merged series 境界 anchor・host 選択・stitch | approved | [aut-02-region-replace-boundary-design.md](./aut-02-region-replace-boundary-design.md) |
| DESIGN-034AUT03B1wG9hW1zS0Yf4 | AUT-03 Loop 録音 1 Pass — Write Range 半開区間閉鎖・境界 flush・明示 Rearm 設計 | approved | [aut-03-loop-write-one-pass-design.md](./aut-03-loop-write-one-pass-design.md) |
| DESIGN-034AUT04CvbbR3Qzx1aKwY | AUT-04 Automation Curve 非破壊保持・Viewport LOD・有界記録 Event 予算 設計 | approved | [aut-04-automation-curve-lod-record-budget-design.md](./aut-04-automation-curve-lod-record-budget-design.md) |
| DESIGN-034RoWWK4Xx4NaZSoBYZSX | Synvelis 独立 CI — Lane 構成・証跡・依存アクセス設計 | approved | [ci-synvelis-pr-push-regression-gate.md](./ci-synvelis-pr-push-regression-gate.md) |
| — | Synvelis — Configurable PPQN / Resolution-independent Musical Time | — | [configurable-ppqn-musical-time-architecture.md](./configurable-ppqn-musical-time-architecture.md) |
| DESIGN-034CTL01Qy1wZt7aC3dE4fGh | CTL-01 ControlOwner/AutomationTimeScope 設計 — Schema v7 Migration | approved | [ctl-01-control-owner-time-scope-design.md](./ctl-01-control-owner-time-scope-design.md) |
| DESIGN-034QAlPiE7atUoGKXNG7Lr | ED-A Editing Model 設計 — Selection/Tool/Snap/Gesture/LinkedContext headless PoC | approved | [ed-a-editing-model-design.md](./ed-a-editing-model-design.md) |
| DESIGN-034QN0IIBPRdrHiOv3uTvq | ED-B media-domain editing design | approved | [ed-b-media-domain-editing-design.md](./ed-b-media-domain-editing-design.md) |
| DESIGN-0Gw01GateWayContract01 | GW-01 Gateway Contract — Capability Registry・Typed Invocation・Versioned Result/Error | draft | [gw-01-gateway-contract-design.md](./gw-01-gateway-contract-design.md) |
| DESIGN-0Gw02AuthorizationPo01 | GW-02 Principal / Execution Grant / Authorization Policy — Fixed/Dynamic Scope | draft | [gw-02-authorization-policy-design.md](./gw-02-authorization-policy-design.md) |
| DESIGN-7Y20yWdQNliSve36HXn2Hy | Issue #145 ci.yml manual-only 設計 — trigger 撤去の最小差分と不変契約の境界 | approved | [issue-145-ci-manual-only.md](./issue-145-ci-manual-only.md) |
| DESIGN-034RIJlv2BuYKx5KG2VyDg | Issue #147 sccache キャッシュ分離・段階展開設計 | approved | [issue-147-sccache-compile-cache.md](./issue-147-sccache-compile-cache.md) |
| DESIGN-034SVCIOPT02kX3pLwQs | Issue #148 変更影響分類・Fail-Closed evidence Gate 設計 | approved | [issue-148-impact-classification.md](./issue-148-impact-classification.md) |
| DESIGN-034RGXfK1v0mkpD0U2xAaN | Issue #151 CI ui-verify lane 設計 | approved | [issue-151-ui-ci-lane.md](./issue-151-ui-ci-lane.md) |
| DESIGN-034RUkhVNYe0G4XOqwwqOa | Issue #157 rev.3 matrix lane skip placeholder — evidence gate 設計 | approved | [issue-157-matrix-placeholder-gate.md](./issue-157-matrix-placeholder-gate.md) |
| DESIGN-034SYN166CiSuMmRy | Issue #166 Full CI 化・ci-summary 統一 — 第一段階設計 | proposed | [issue-166-full-ci-ci-summary.md](./issue-166-full-ci-ci-summary.md) |
| DESIGN-RBfO0jdqOYWxb6xvtT5z0p | Issue #167 Tracker reconcile 設計 — Consumer 共有 checkout 適合・mode 分離・単一 pin 契約 | approved | [issue-167-tracker-reconcile-entry.md](./issue-167-tracker-reconcile-entry.md) |
| DESIGN-034U2uVeVKqECLpCn8kbSa | Issue #184 CI verify/evidence 失敗 — canonical temp fixture 設計 | approved | [issue-184-ci-verify-evidence-canonical-temp-fixture.md](./issue-184-ci-verify-evidence-canonical-temp-fixture.md) |
| DESIGN-yiV0z7HH1jyKUmweFh9hCT | L1 Design — Safe Import・Verified Media・Lowering・Headless Driver | approved | [l1-local-wav-verified-playback-design.md](./l1-local-wav-verified-playback-design.md) |
| DESIGN-034QmfChvqgB632y8fAydk | L2 Design — Render Snapshot・Preflight・Sequential Render・WAV Output・Reopen Verify・Safe Publish | approved | [l2-master-sequential-offline-wav-export-design.md](./l2-master-sequential-offline-wav-export-design.md) |
| DESIGN-YFKePNnF4peFnskSfrnSc2 | MD-A Design — TakeGroup/Comp スキーマ・Effective Comp 投影・正規化 Placement/Playback 時間契約 | approved | [mda-comp-effective-projection-placement-design.md](./mda-comp-effective-projection-placement-design.md) |
| — | Synvelis × Ruviform — Media / Import / Export / Offline Render / UI / Diagnostics 統合アーキテクチャ | — | [media-import-export-offline-render-ui-diagnostics-architecture.md](./media-import-export-offline-render-ui-diagnostics-architecture.md) |
| DESIGN-034MSA52Mn8xRt6yQwZkPf | 最小 MIDI Source / Note 編集契約 設計 | draft | [midi-source-note-editing-design.md](./midi-source-note-editing-design.md) |
| DESIGN-034PCA03Nj8xRs4yAbOcDe | PC-A Parameter Control Model 設計 — Scope/Breakdown/StateMachine headless PoC | approved | [pc-a-parameter-control-design.md](./pc-a-parameter-control-design.md) |
| DESIGN-034PCB03Mn8xRs4uZvWdEf | PC-B Parameter Control dispatch/note-scope/RT/offline 設計 | approved | [pc-b-parameter-dispatch-note-scope-design.md](./pc-b-parameter-dispatch-note-scope-design.md) |
| DESIGN-034RoTLCvNiYIgUG0JDqVa | PC-C1 Recording Transaction prepare/apply/finalize 設計 | approved | [pc-c1-recording-transaction-design.md](./pc-c1-recording-transaction-design.md) |
| DESIGN-034Up3Bm4bb2Rb9RZ1ien7 | PC-C2 Control Relation Index 設計 — 双方向辺モデル・状態証拠・有界 traversal | approved | [pc-c2-control-relation-index-design.md](./pc-c2-control-relation-index-design.md) |
| DESIGN-034UqtGwJKTXdIjOxuLVS2 | PC-C3 ControlTrace 設計 — stage evidence slot・freshness 判定・plan activation 投影 | approved | [pc-c3-control-trace-design.md](./pc-c3-control-trace-design.md) |
| DESIGN-AkFUh4M60YuHo8COCvb0Um | PC-C5 slice1 Relation Inspector 設計 — RelationRow/RelationView/RelationNav + Inspector 第 2 Table | approved | [pc-c5-01-relation-inspector-design.md](./pc-c5-01-relation-inspector-design.md) |
| — | Synvelis × Ruviform — Playback / Mixer / Generic Execution Graph Architecture | — | [playback-mixer-generic-execution-graph-architecture.md](./playback-mixer-generic-execution-graph-architecture.md) |
| — | Synvelis — Project / Asset / Persistence Architecture | — | [project-asset-persistence-architecture.md](./project-asset-persistence-architecture.md) |
| DESIGN-Bz2IMMfywvmEjmdIeThNvq | RCD-01 Input Adapter — binding/Arm/Monitor/channel-map と fake port fail-stop | approved | [rcd-01-input-adapter-binding-arm-monitor-fake-port.md](./rcd-01-input-adapter-binding-arm-monitor-fake-port.md) |
| DESIGN-034SF01Mo9rSt4uV1wXyZaCdE | SF-01 Semantic Routing Model 設計 — 型・整合・Schema v6 Migration | approved | [sf-01-semantic-routing-model-design.md](./sf-01-semantic-routing-model-design.md) |
| DESIGN-034SF02Vy9qNo6pS9uA4bCeFg | SF-02 RoutingTransaction/TempRef Staging・二層検証・Mute/Solo Audibility 設計 | approved | [sf-02-routing-transaction-design.md](./sf-02-routing-transaction-design.md) |
| DESIGN-106SF03Ef3fGh4iJk5lMn6oPq | SF-03 slice1 — Lowering/SourceMap/Activation 実装設計 | approved | [sf-03-slice1-lowering-sourcemap-activation-design.md](./sf-03-slice1-lowering-sourcemap-activation-design.md) |
| DESIGN-106SF03Dz2aBc3dEf4gHi5jKl6 | SF-03 slice2 設計 — RoutingFault 18分類・Last-good/Retry・reconciliation | approved | [sf-03-slice2-fault-reconcile-design.md](./sf-03-slice2-fault-reconcile-design.md) |
| DESIGN-034UfwVzxaw6WwUYS3nMDN | SF-03 slice3 設計 — processor host 接続 boundary・世代連動 teardown | approved | [sf-03-slice3-plugin-host-connect-design.md](./sf-03-slice3-plugin-host-connect-design.md) |
| DESIGN-6tbKfWyun38H7ERAnefMsA | SF-03 slice4 設計 — NoRebuild Equivalence・世代連動 processor control 払い出し | approved | [sf-03-slice4-act-acceptance-design.md](./sf-03-slice4-act-acceptance-design.md) |
| DESIGN-034Uj7LfhkkACvyn53L7eN | SF-03 slice5 設計 — session EnginePort・SessionRoutingEngine・canonical probe | approved | [sf-03-slice5-session-engine-connect-design.md](./sf-03-slice5-session-engine-connect-design.md) |
| DESIGN-034Um5591oRMfPg7cVthyU | SF-04 slice1 設計 — Semantic Probe Adapter（project contract + player runtime adapter） | approved | [sf-04-slice1-semantic-probe-adapter-design.md](./sf-04-slice1-semantic-probe-adapter-design.md) |
| DESIGN-lW54zR76NmEm8cc3NlMskX | SV-GUI-01 Design — Desktop App Bootstrap・Composition Root・Frontend Asset Pipeline | approved | [sv-gui-01-desktop-app-bootstrap-design.md](./sv-gui-01-desktop-app-bootstrap-design.md) |
| DESIGN-034RAAXZ1XMIuk2zr2dr4w | SV-GUI-02 Frontend View Model・Binding IPC・Reference Workspace View | approved | [sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md](./sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md) |
| DESIGN-5n53fkGhYaUtJlMHOu5P1p | SV-GUI-03 slice1 Transport/Inspector 設計 — MainScreenState push + 既存 syn.control.* 経路 | approved | [sv-gui-03-01-transport-inspector-slice1-design.md](./sv-gui-03-01-transport-inspector-slice1-design.md) |
| DESIGN-8o9zUv4L5ae0FBB0zOM293 | SV-GUI-03 slice2 Arrangement/Undo 設計 — workspace read model + syn.project.* op 露出 | approved | [sv-gui-03-02-arrangement-undo-slice2-design.md](./sv-gui-03-02-arrangement-undo-slice2-design.md) |
| DESIGN-034UogNAUQrEmBkgZnhrAk | SV-GUI-04 slice1 precursor 設計 — HeadlessWindowHost + dock fixture | approved | [sv-gui-04-01-headless-window-handle-dock-design.md](./sv-gui-04-01-headless-window-handle-dock-design.md) |
| DESIGN-Ywl9hP3wdlHWjoMPB7Hvjn | SV-GUI-04 slice1 設計 — 2nd view spec + env-gated 2窓 mount + FakeDriver fixture | approved | [sv-gui-04-01-two-window-slice1-design.md](./sv-gui-04-01-two-window-slice1-design.md) |
| DESIGN-034UqwNDJrureYsAqmckuw | SV-GUI-04 slice2 設計 — close_guard op + 末尾 window veto | approved | [sv-gui-04-02-close-veto-design.md](./sv-gui-04-02-close-veto-design.md) |
| DESIGN-034UqNokCXcUXgKk1tyv0p | SV-GUI-04 slice2 dock transfer: workspace adoption と dock/UI 権威分界の詳細設計 | approved | [sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md](./sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md) |
| DESIGN-BWzJVIfJHZS8suU7aFFAGf | SV175 Native App Shell 設計 — NativeShell routing・ViewBinding adapter・native feature lane | approved | [sv175-native-shell-design.md](./sv175-native-shell-design.md) |
| DESIGN-034T728YcMrGoBwDtbkIqS | SV177 Product Theme / Workspace Preset — synvelis-theme crate・DTCG overrides・slot-based preset・semantic screen spec | approved | [sv177-product-theme-workspace-preset-design.md](./sv177-product-theme-workspace-preset-design.md) |
| DESIGN-Xk9mQ2vR7wTzLp4nHs8dFj | SV185-01 mixer.gain projection 設計 — ControlModel bootstrap・wire target・display_value | approved | [sv185-mixer-gain-projection-design.md](./sv185-mixer-gain-projection-design.md) |
| DESIGN-43KJVuVRlioN1JtUFVuqtW | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界 | approved | [sv28-ruviform-pin-cc7d5296.md](./sv28-ruviform-pin-cc7d5296.md) |
| DESIGN-034T2aGDbNinkumWeftaiP | SYN-IO-02 DirectoryBundleStore StagedFile 移行設計 | approved | [syn-io-02-directory-bundle-store-stagedfile-design.md](./syn-io-02-directory-bundle-store-stagedfile-design.md) |
| DESIGN-034RBzdpo91wcbqBK7Gb9t | SYN-OBS-01 Design — Host所有 Logging Owner・設定解決・Source Catalog | approved | [syn-obs-01-design-host-logging-owner-source-catalog.md](./syn-obs-01-design-host-logging-owner-source-catalog.md) |
| DESIGN-034T2RF305WAVSTAGED00Y | SYN-RF305 WAV Export StagedFile 移行設計 | approved | [syn-rf305-wav-export-staged-file-design.md](./syn-rf305-wav-export-staged-file-design.md) |
| — | Synvelis-driven Ruviform Target Architecture v0.7.1 | — | [synvelis-driven-ruviform-target-architecture-v07.md](./synvelis-driven-ruviform-target-architecture-v07.md) |
| — | Synvelis × Ruviform — Time / Transport / MIDI / Automation / Plugin Integration Architecture v0.5 | — | [time-transport-midi-automation-plugin-integration-architecture.md](./time-transport-midi-automation-plugin-integration-architecture.md) |
| DESIGN-034QdO7AB6cp4DRgKeuuEn | VS-A Project/Recording Foundation — Stable ID・時間型・Command・永続化 PoC | approved | [vs-a-project-recording-foundation-stable-id-command-poc.md](./vs-a-project-recording-foundation-stable-id-command-poc.md) |
| DESIGN-8OW4JtYUYaLPOTZ9ZGxXuz | VS-B Clock / Gate Scheduler / RT Capture — Mock Input・Buffer Pool・SPSC | approved | [vs-b-clock-gate-rt-capture-mock-spsc.md](./vs-b-clock-gate-rt-capture-mock-spsc.md) |
| DESIGN-034QfL0bH9u3cVqK0pYwT7x | VS-C Staging Writer / Journal / Crash Recovery — Verified Media Range | approved | [vs-c-staging-writer-journal-crash-recovery-verified-media-range.md](./vs-c-staging-writer-journal-crash-recovery-verified-media-range.md) |
| DESIGN-sbd87tLPXRrvP2kyru1XD2 | VS-D Recording Commit / E2E — Idempotent Commit・Undo・Reload 設計 | approved | [vs-d-recording-commit-e2e-idempotency-undo-reload.md](./vs-d-recording-commit-e2e-idempotency-undo-reload.md) |
| — | Synvelis Workspace / Docking / Multi-Window 統合設計 | — | [workspace-docking-state-architecture.md](./workspace-docking-state-architecture.md) |
| DESIGN-034QwYMaHNxxpkZgLxPjx6 | WS-A Workspace 基盤設計 — View/Selection/LinkGroup/Command/Dock headless PoC | approved | [ws-a-workspace-foundation-design.md](./ws-a-workspace-foundation-design.md) |
| DESIGN-HEDK2Gt2m6FAc3GREIhJDl | X Design — Canonical publication receipt・commit digest 接続・同一経路 playback/export | approved | [x-recorded-verified-source-playback-export-design.md](./x-recorded-verified-source-playback-export-design.md) |
