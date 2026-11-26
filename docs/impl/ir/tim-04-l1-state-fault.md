# TIM-04 L1 state/fault 実装記録

調査で依存 crate bin は downstream test から CARGO_BIN_EXE でも artifact 保証でも利用できないことを確認した。最小の consumer-side composition runner を採用した。VST3 descriptor id は vendored vst3 uid byte order を再現し、CLAP descriptor は upstream fixture contract と一致させた。VST3 blob は component/controller framed RFST、CLAP blob は raw RFST として独立 parser で検査する。restore は mutated HEAD ではなく initial manifest explicit restore が必要だった。hang は landing だけでは不十分で、wedged audio thread へ follow-up submit してから state command timeout を観測する。
