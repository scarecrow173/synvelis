# TIM-04 L1 state/fault 仕様

目的: ruviform real ABI test_stateful/test_fault の VST3/CLAP を L1 out-of-process で駆動し、PluginStateHost/Session/Store の save/restore、capture quality、fault projection、旧 blob 保護を検証する。対象は consumer-run のみで、製品 registry/discovery、Offline render 実組込み、GUI、市販 plugin、AudioLens、3OS 最終受入は含めない。

受入 mapping: issue #128 の仮想優先 DoD のうち L1 real ABI state/fault を前進させる。BestEffort を exact に昇格しない。Unsupported を正直に記録する。
