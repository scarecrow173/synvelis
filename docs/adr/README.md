# Architecture Decision Records

<!-- doc-suite:generated-index -->

Directory: `docs/adr`

| ID | Title | Status | File |
| --- | --- | --- | --- |
| ADR-qjrwwGin4E0PE4ksCSLysY | ALIB-01 — synvelis-library crate 配置と Provider 契約境界 | accepted | [alib-01-synvelis-library-crate-boundary.md](./alib-01-synvelis-library-crate-boundary.md) |
| ADR-034AUT02Xy4mKp8wQe2sVn7j | AUT-02 境界 Sentinel/Anchor 配置 — merged series 単位の境界復元・host region 選択 | accepted | [aut-02-boundary-sentinel-anchor-placement.md](./aut-02-boundary-sentinel-anchor-placement.md) |
| ADR-034AUT0312NAA2Mk53Gle5 | AUT-03 Write Range 半開区間の表現 — 境界 flush・coverage_end・Pass fixation | accepted | [aut-03-write-range-boundary-flush-pass-fixation.md](./aut-03-write-range-boundary-flush-pass-fixation.md) |
| ADR-034AUT04yQRFZlLO2ytJpt | AUT-04 Viewport LOD・有界記録キューの表現 — model 層 LOD 投影・RT SPSC・型付き喪失 provenance | accepted | [aut-04-viewport-lod-bounded-record-queue.md](./aut-04-viewport-lod-bounded-record-queue.md) |
| ADR-034CTL01Px0vYs6zB2cD3eFg | CTL-01 ControlOwner/AutomationTimeScope 責務分離・Schema v7・Versioned Migration 境界 | accepted | [ctl-01-control-owner-time-scope-schema-v7.md](./ctl-01-control-owner-time-scope-schema-v7.md) |
| ADR-034QAmdUp9fg7PPVCOQtcx | ED-A editing-model placement: synvelis-workspace gesture/tool/snap state machines + synvelis-project batch ops | accepted | [ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md](./ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md) |
| ADR-034QN06oqUD5VvvsC9aedM | ED-B media-domain gesture positions + slip/trim/marquee commit semantics | accepted | [ed-b-media-domain-gesture-positions-slip-trim-marquee-commit-semantics.md](./ed-b-media-domain-gesture-positions-slip-trim-marquee-commit-semantics.md) |
| ADR-Gw01GateWayCrateBoundary | GW-01 — synvelis-gateway crate 配置と契約境界 | accepted | [gw-01-gateway-contract-crate-boundary.md](./gw-01-gateway-contract-crate-boundary.md) |
| ADR-qs0t3aMSfTG19Vm5F4r2fp | Issue #145 ci.yml manual-only 化 — pull_request/push trigger を撤去し workflow_dispatch 専用にする | accepted | [issue-145-ci-manual-only.md](./issue-145-ci-manual-only.md) |
| ADR-034RUwki0c6svA0PlhUBB8 | Issue #157 rev.3 skip placeholder 受理方式 — gate 名照合拡張 vs workflow 構造変更 | accepted | [issue-157-rev-3-skip-placeholder-gate-vs-workflow.md](./issue-157-rev-3-skip-placeholder-gate-vs-workflow.md) |
| ADR-XTlIzRk1SB1nTV82KYSW6m | Issue #167 Tracker reconcile 入口 — 共有 pin eed590e で RF 別 workflow reconcile 契約を Consumer 採用(trigger 禁止の補足契約の転用) | accepted | [issue-167-tracker-reconcile-entry.md](./issue-167-tracker-reconcile-entry.md) |
| ADR-Ym8S8syVSNCeQiMNBHHfHt | L1 — Import・Blob・Lowering・Driver の配置と境界 | accepted | [l1-player-slice-boundaries.md](./l1-player-slice-boundaries.md) |
| ADR-034QmbgsQDkLDllHgAxa1f | L2 — Render slice boundaries・Ruviform sequential offline driver adoption・Staged publish | accepted | [l2-render-slice-boundaries.md](./l2-render-slice-boundaries.md) |
| ADR-cyMsJVD4HiJceSTYXBJCJD | MD-A — Clip 正規化 Placement/Playback スキーマ・Comp 採用モデル・Effective Comp 投影の境界 | accepted | [mda-normalized-placement-comp-schema.md](./mda-normalized-placement-comp-schema.md) |
| ADR-034PCA02Mh7wQr3xZaNbCd | PC-A parameter-control placement: synvelis-project persisted model + evaluation, synvelis-workspace session state machines | accepted | [pc-a-parameter-control-placement-project-model-workspace-session.md](./pc-a-parameter-control-placement-project-model-workspace-session.md) |
| ADR-034PCB02Lm9wQr3tYuVbCd | PC-B: dispatch 境界・note entity・prepared eval の配置決定 | accepted | [pc-b-dispatch-boundary-note-entity-prepared-eval.md](./pc-b-dispatch-boundary-note-entity-prepared-eval.md) |
| ADR-034RoTL4YIadCWevIM5Mfg | PC-C1: commit 境界を session CAS orchestration + 非破壊 prepared commit に置く決定 | accepted | [pc-c1-commit-session-cas-orchestration-prepared-commit.md](./pc-c1-commit-session-cas-orchestration-prepared-commit.md) |
| ADR-034Up0Uogue2QzCYAOA09T | PC-C2 Control Relation Index — revision-tagged 派生双方向 projection と enabled-only cycle 意味論 | accepted | [pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md](./pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md) |
| ADR-pc5c501r3lUiC0v3rag3 | PC-C5 slice1: Relation 投影は workspace read model + verbatim coverage、index は query 毎 rebuild、探索は depth-1 hop + anchor nav-stack | accepted | [pc-c5-01-relation-readmodel-coverage.md](./pc-c5-01-relation-readmodel-coverage.md) |
| ADR-034SF01Ln8qRs3tU0vWxYzBcD | SF-01 Routing Model 配置・Schema v6 Migration・Reserved Well-Known ID | approved | [sf-01-routing-model-schema-v6-migration.md](./sf-01-routing-model-schema-v6-migration.md) |
| ADR-034SF02Ux8pMn5oR8tY3zAbDe | SF-02 RoutingTransaction 配置・TempRef Staging・channels 永続化・Mute/Solo 権威・汎用検証の再利用 | accepted | [sf-02-routing-transaction-port-channels-mute-solo-authority.md](./sf-02-routing-transaction-port-channels-mute-solo-authority.md) |
| ADR-106SF03Cd2eFg3hIj4kLm5nOp | SF-03 slice1 — Runtime ID は SourceMap 経由のみ・Activation は RT 観測証拠で分離 | approved | [sf-03-semantic-lowering-source-map-activation-boundary.md](./sf-03-semantic-lowering-source-map-activation-boundary.md) |
| ADR-034RAA8oZVNDaL6mMAMJ55 | SV-GUI-02 syn.* binding IPC surface・notify outbox・bounded wire DTO | accepted | [sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md](./sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md) |
| ADR-034Ul4shgS8qAsLvnSHlkQ | SV-GUI-03 slice1: Transport/Inspector の表示値は app 層 MainScreenState push で供給し domain/ops を変更しない | accepted | [sv-gui-03-01-transport-inspector-app-state-push.md](./sv-gui-03-01-transport-inspector-app-state-push.md) |
| ADR-WWJ1nzLLj2Q6WEjt7ZQg3t | SV-GUI-03 slice2: Arrangement rows は workspace read model + MainScreenState push、Undo/Redo は syn.project.* op 露出 | accepted | [sv-gui-03-02-arrangement-readmodel-undo-ops.md](./sv-gui-03-02-arrangement-readmodel-undo-ops.md) |
| ADR-G3aQRfIh47FQ5jU2AaDhjd | SV-GUI-04 slice1: 2nd window は shell.attach_view + create_view_window 経路、view-2 は transport deck spec | accepted | [sv-gui-04-01-two-window-second-view.md](./sv-gui-04-01-two-window-second-view.md) |
| ADR-vXwuA1OrALJD99tNRgZ8gQ | ADR: Native App Shell は Driver-neutral shell + GPUI 標準 Driver。SV-GUI-01 の WebView/SolidJS 決定を supersede | accepted | [sv175-native-shell-driver-neutral-gpui.md](./sv175-native-shell-driver-neutral-gpui.md) |
| ADR-034T6XUbzRkbVBGiEFXH4Q | SV177 product theme crate boundary, DTCG document envelope and slot-based workspace preset schema | accepted | [sv177-product-theme-crate-boundary-dtcg-document-envelope-and-slot-based-workspa.md](./sv177-product-theme-crate-boundary-dtcg-document-envelope-and-slot-based-workspa.md) |
| ADR-034RBxHj0wzQculGmgXguq | SYN-OBS-01 単一crate ruviform-observability logging feature 採用と Host 所有 Global Subscriber | accepted | [syn-obs-01-crate-ruviform-observability-logging-feature-host-global-subscriber.md](./syn-obs-01-crate-ruviform-observability-logging-feature-host-global-subscriber.md) |
| ADR-034T2RF305WAVSTAGED01C | SYN-RF305 — WAV Export Consumer-first StagedFile cutover と publish durability 写像 | accepted | [syn-rf305-consumer-first-staged-file-cutover.md](./syn-rf305-consumer-first-staged-file-cutover.md) |
| ADR-034QdxBCx9w88ptCJEEz7D | VS-A Workspace構成・Ruviform git依存・永続化PoC・Stable ID生成 | accepted | [vs-a-workspace-ruviform-git-poc-stable-id.md](./vs-a-workspace-ruviform-git-poc-stable-id.md) |
| ADR-ODnVCNLDT5mtpswKxVDw9R | VS-B SPSCプリミティブ・Gate Schedule表現・Fault通知・Chunk所有権 | accepted | [vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md](./vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md) |
| ADR-034QfJdymYmhzfa9PFs8fD | VS-C Journal形式・Checkpoint Sync方針・Fault隔離・Finalized出力 | accepted | [vs-c-journal-checkpoint-sync-fault-finalized.md](./vs-c-journal-checkpoint-sync-fault-finalized.md) |
| ADR-MSyE0rkQSGNdJNwNrTVoCy | VS-D CommitIdentity・Manifest・Single Mutation・Undo 単位 | accepted | [vs-d-commit-identity-manifest-single-mutation.md](./vs-d-commit-identity-manifest-single-mutation.md) |
| ADR-034QwWcA5mHzwXYyFiDTTi | WS-A workspace crate boundary and Ruviform dock/host contract adoption | accepted | [ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md](./ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md) |
| ADR-yEJFG31sy193FI2AeehiqF | X — Finalized raw media の canonical publication 境界・commit digest 接続・playback 無変更 | accepted | [x-recorded-media-canonicalization-boundary.md](./x-recorded-media-canonicalization-boundary.md) |

## Settled <!-- doc-suite:settled-index -->

| ID | Title | Status | File |
| --- | --- | --- | --- |
| ADR-2kAD08Yeuxmx8U4t2mD2wr | SV-GUI-01 app crate boundary・webview feature gate・vendored ui/* TS pipeline | superseded | [sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md](./sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md) |
