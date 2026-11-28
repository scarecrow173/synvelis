# TIM-04 L1 state/fault テスト仕様

`tim04_l1_abi_state.rs` は 4 本。1) scan/lifecycle/capture quality matrix: VST3/CLAP で processing=BestEffort、stopped stable boundary=Coordinated、test_fault=Unsupported。2) stateful save→mutate→restore→save: RFST length/checksum/gain、sha256、blob length、manifest receipt、delivered-audio ordering を oracle とする。3) abort/exit42/hang: Crashed/Crashed/Timeout 投影、notification exit/cause、diagnostics、fail-closed restore、Strict QualityRejected、旧 blob bytes/digest/HEAD/manifest 不変を検査する。4) permission fault matrix: actual RFST bytes を使い store denial / readback failure を検査する。証拠は JSONL の case/driver/verdict/detail/observed/environment schema。

## 実行契約

`tim04_l1_abi_state.rs` の 4 tests は test-only `tim04-l1-plugin-runner` と VST3/CLAP fixture cdylib を必要とする artifact-dependent tests である。generic workspace-wide / MSRV lane の対象外とし、正規実行は fixture artifacts と runner を先に build する dedicated opt-in lane `mise run test-tim04-l1` とする。coverage は削除せず、同 laneが `--ignored` で 4 tests を実行する。
