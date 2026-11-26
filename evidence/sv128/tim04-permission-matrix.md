# TIM-04 permission fault matrix evidence — synvelis#128 slice

本ファイルは TIM-04 permission fault matrix sliceの証拠サマリである。machine-readable evidenceは `tim04_l1_abi_state.jsonl`、canonical verificationは `verification-logs/` を参照する。

## 環境

- OS/Arch: Windows / x86_64
- isolation: `out-of-process/l1`
- fixture: ruviform `test_stateful` / `test_fault` VST3・CLAP
- ruviform resolved rev: `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`（本 sliceでは pin を変更していない）
- store backend: test-only in-memory fault-injecting `PluginStateStore`
- evidence生成run: `SYNVELIS_TIM04_L1_EVIDENCE_DIR=evidence/sv128 mise exec -- cargo test -p synvelis-player --test tim04_l1_abi_state l1_permission_fault_matrix_real_rfst -- --nocapture`（補助証拠。canonical laneは `verification-logs/summary.txt`）

## real ABI capture input

matrixへ入力した actual RFST bytesは次のとおりである。capture自体の成功を permission denialのPassへ転用せず、同じ bytesを fault-injecting store matrixへ入力した。

| format | len | SHA-256 | RFST parse |
| --- | ---: | --- | --- |
| VST3 | 48 | `65a76e693fc229758977b539f355e9974b7bdaeda236aaf4e470f069fb91006e` | gain `1.0` / checksum `1644390705` |
| CLAP | 20 | `5951a387e30e669a21ba5e8078ac8d79c14f22593513d7702fb68383fae1a022` | gain `1.0` / checksum `1644390705` |

VST3 blobは framed RFST、CLAP blobは raw RFST streamであり、両方とも parserで version/gain/checksumを検査した。

## Matrix verdict

JSONLの permission matrix rowsは **30件** で、集計は **Pass 28 / Unsupported 2 / Fail 0 / NotRun 0**。

host/store-side denial rowsはVST3/CLAPそれぞれ次を独立 caseとして持つ。

- save: `BlobPublish`、`ManifestPublish`、`HeadUpdate` × Strict/Standard
- restore/read: `HeadRead`、`ManifestRead`、`BlobRead`
- policy: processing中の real BestEffort captureで Strictがstore未接触の `QualityRejected`、Standardがtyped `ManifestPublish` denialへ到達
- reconcile: indeterminate blob publish + denied blob readback、indeterminate HEAD update + denied HEAD/manifest readback

各行は typed operation/effect、before/after HEAD、manifest setとbytes digest、blob digest、reconcile resultを `observed` に保持する。save denialでは旧HEAD・manifest set・既存blob bytes/SHA-256が変わらず、restore denialは黙った成功または空state適用にならなかった。

## plugin-originated capability verdict

`vst3_plugin_originated_permission_capability` と `clap_plugin_originated_permission_capability` は **Unsupported** である。現行 ruviform pinの `test_fault` fault bandsは `none/abort/exit/hang/error/rt_alloc/bad_time/invalid` で permission bandを持たず、`HostError` に PermissionDenied variantはなく、`test_fault` の state captureは `Unsupported` だった。したがって plugin-originated PermissionDeniedの real ABI Pass/Fail evidenceは存在しない。mockの `PluginStateFailureKind::PermissionDenied` は type boundary testにのみ使用し、real ABI Passへ転用していない。

## fail-closed oracle

- HEAD/manifest read denialは session-level typed error。
- blob read denialは per-record `StoreFailed { operation: BlobRead, kind: PermissionDenied }`。
- plugin-port refusalは `Failed` であり、store blob read denialと同じ outcome shapeへ潰していない。
- indeterminate publish + denied readbackは `ReadbackFailed` であり、成功とも確定失敗とも偽らない。
- 実FS ACL/locking probeは Required evidenceにしていない。deterministic store seamがOS-L1 contract evidenceである。

## non-claims / 残件

- plugin-originated PermissionDeniedを実測Passにする ruviform fixture/HostError capability
- Offline render実plugin組込みと `TIM04-RCP-01` end-to-end
- typed runtime process oracle、製品registry/discovery配線
- GUI/Native Editor、AU、市販plugin、物理機材、AudioLens cross-gate、3OS最終受入
- docs index（Coordinatorが統合時に整備）
