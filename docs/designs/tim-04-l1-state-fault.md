# TIM-04 L1 state/fault 設計

runner 決定: upstream runner は依存 crate の bin が downstream にビルド保証されず、CARGO_BIN_EXE も使えないため正規 reuse できない。Synvelis test-only composition runner `examples/tim04_l1_plugin_runner.rs` を明示 bin target とし、実 fixture cdylib を production host bridge で load して `ruviform_plugin_export::run` へ渡す。no-argv contract を守る。Cargo の bin は dev-dependency を使えないため bridge/export edge は通常 dependency とした。これは同一パッケージ内で deterministic artifact を保証する最小構成である。

state 経路: PluginHost register_out_of_process → Scan/Instantiate/Initialize/Activate/StartProcessing → PluginStateHost → PluginStateSession → DirectoryPluginStateStore。mutation は delivered audio を待ってから state verb を送る。fault は Dedicated child のみで arm する。

failure typing: PluginStateHost は Failed/Ipc に加え、dead instance 上の Unsupported を lifecycle/notification/diagnostics の FailureCause で attribution する。Timeout のみ Timeout、それ以外は Crashed。通知 drain は adapter が failed state command の attribution 時だけ行い、handle 別に cache して他 handle notification を捨てない。
