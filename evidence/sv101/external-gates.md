# PC-C4 plugin binding external gates (Issue #101 / synvelis#101)

headless Fixture/テストの成功を実機 Pass にしないための外部 Gate
判定記録。

| Gate | 対象 AC | 状態 | 理由 |
| --- | --- | --- | --- |
| 実 VST3/AU/CLAP plugin backend での Parameter Binding | PLUG-C4-01〜04 | **NotRun** | headless `RecPlugin`/`PluginHost` fixture のみ。実 plugin の `ParamInfo` 列挙・write means・state 復元を伴う binding は ruviform#253（実 OS/plugin backend gate）解消後の検証が必要 |
| 実 OS device backend / 実機 audio device | PLUG-C4-01/05 | **NotRun** | RT 経路は `GraphRuntime` + `PluginNode` の in-process fixture。実デバイスの clock/latency/underflow 挙動は未観測 |
| plugin GUI 値変更 → parameter authority 反映 | PLUG-C4-04 | **NotRun** | GUI は synvelis#114 系の別 gate。headless は project 権威の再解決のみ検証 |
| AudioLens f64/RT contract | PLUG-C4-05 | **NotRun** | Ruviform#294 系。本 slice は f64 値経路に触れない |
| wire-schema / L1 op surface | PLUG-C4-06 系 | **NotRun** | `syn.control.*` op の wire 検証は別 slice の責務 |
| Domain Oracle（synvelis#103） | PLUG-C4-01〜05 | **NotRun** | oracle 行の独立検証は #103 の責務。`pc_c6_oracle_pl` は oracle 風 fixture であり独立 oracle ではない |
| Ruviform #307 attach-time FrameBase/Clock 互換 | PLUG-C4-01/05 | **契約証拠のみ** | `AttachClockContext`/`check_frame_base`/`ParamDispatch::new` の frame-base 検査を fixture で被覆。upstream 実機互換は NotRun |
| Ruviform cross-consumer CI 非回帰 | PLUG-C4-05 | **NotRun** | 本 slice は synvelis 側 consumer のみ変更。Ruviform cross-consumer lane は upstream CI の責務 |
| OS-L1 native validation（Windows/macOS/Linux） | 全般 | **部分的** | Windows headless lane は実施。macOS/Linux OS-L1・GUI/EXT は POLICIES 上 deferred（GUI/EXT capability 未提供のため） |

## 判定ルール

- headless テストの green は「headless 契約の Pass」であり、
  実機・実 plugin・GUI・Oracle の Pass を意味しない。
- `NotRun`/`Unsupported` は「未実施」または「本タスク範囲外」
  を意味し、推測で Pass にしない。
