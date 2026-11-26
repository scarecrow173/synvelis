# TIM-04 L1 state/fault テスト仕様

`tim04_l1_abi_state.rs` は 3 本。1) scan/lifecycle/capture quality matrix: VST3/CLAP で processing=BestEffort、stopped stable boundary=Coordinated、test_fault=Unsupported。2) stateful save→mutate→restore→save: RFST length/checksum/gain、sha256、blob length、manifest receipt、delivered-audio ordering を oracle とする。3) abort/exit42/hang: Crashed/Crashed/Timeout 投影、notification exit/cause、diagnostics、fail-closed restore、Strict QualityRejected、旧 blob bytes/digest/HEAD/manifest 不変を検査する。証拠は JSONL の case/driver/verdict/detail/observed/environment schema。
