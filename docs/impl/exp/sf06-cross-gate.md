---
id: "EXP-161wiYPxGFy8V5QGwDmnaX"
title: "SF-06 cross-gate headless fixture 被覆棚卸し — experiment log (synvelis#109)"
created: "2026-11-15"
updated: "2026-11-15"
---

# SF-06 cross-gate headless fixture 被覆棚卸し — experiment log

## E1. 被覆監査の進め方

Issue #109 の SF-MODEL/VALID/ACT/PROBE case を既存
`synvelis-project`/`synvelis-player`/`synvelis-render` の
`tests/` 全件へ写像した。先行 ledger
（`evidence/reports/sf-06-cross-gate-ledger.md`）は
2026-11-10 計測で、その後 merge された SF-03（lowering/
activation）・SF-04（probe adapter）slice 群の T2 被覆を
反映していなかった。監査は「case の各節がどの既存テスト
または未被覆分岐に対応するか」を先に固定し、重複テストを
追加しない方針で進めた。

監査で見つかった構造的 gap は 1 系統:

- **send/bus 経路の PCM 実測欠落**: SF-03 slice の test
  （`sf03_l1.rs` 等）は send の lowering・source map・
  gain stage の存在を検証するが、pre/post tap が実際に
  異なる信号を運び、bus→master で正しく集約され、mute/solo
  が send 経路へ波及することを **PCM サンプルで実測**する
  fixture がなかった。これは SF-VALID-03/04（mute/solo・
  pre/post 意味論）と SF-ACT-02（signal-level route）の
  case-level 証拠として必要。

## E2. fixture 設計の検討

### 候補 A: project 層の model assert のみ

却下。`sf06_oracle.rs` の audibility sweep が既に model 層の
mute/solo を 2^14 組合せで被覆しており、重複になる。

### 候補 B: lowering source map の assert のみ

却下。`sf03_l1.rs` の `assert_wires_send_tap_into_processor`
等が source map 層を被覆済み。「配線が存在する」ことと
「正しい信号が流れる」ことは別の契約であり、PCM 実測が
必要。

### 候補 C（採用）: `RoutingEngine` render PCM の解析 oracle

`fixture_sample_f32` × dyadic gain の独立 oracle と
`PlayReport.pcm` を `assert_eq` で比較。gain を全て 2 進
（0.5/0.25/1.125/1.75/1.5）に選ぶことで f32 丸めを排除し、
bit 一致を要求できる。tap 識別は fader=0.5 で pre tap
（post-chain/pre-fader = x）と post tap（fader 出力 = 0.5x）
が異なる信号を運ぶことを利用 — pre tap が誤って post 信号を
運ぶ場合の総和（0.875x）は期待値（1.125x）と一致しないため
fail する。

## E3. 実装中の発見

- `RoutingEditIntent` は `CreateBus`+`CreateConnection`+
  `CreateSend` を 1 revision で atomic commit する（SF-02
  契約）。`TempBusId`/`TempSendId`/`TempConnectionId` は
  gesture-local で persisted ID と衝突しない。
- `SendGainSpec::Create` は新規 send に専用 `SendLevel`
  target を seed する。`initial` gain が lowering の gain
  stage に焼き込まれる。
- strip の `Fader` target は `ControlOwner::Strip(strip)` +
  `ParameterRole::Fader` で一意に解決できる
  （`SetParameterBase` で base 値を変更 → lowering が
  post-fader gain として反映）。
- `RoutingEngine` は prepare→publish→run を revision 毎に
  独立して行う設計で、fresh engine per render により
  revision 間の状態持ち越しを排除できる。
- `InMemoryStore` による save→load で `Project` が完全に
  復元され、同一 media root で render すると PCM/CRC が
  bit-for-bit 一致する — persisted state が PCM の権威で
  あることの実測証拠。

## E4. 判定

被覆監査の結果、既存 fixture で被覆済みの case への重複
追加は行わず、PCM 実測が欠落していた send/bus/mute-solo
経路のみ `sf06_send_path_pcm.rs` として追加した。
`evidence/sv109/coverage.md` が case→fixture の正本写像。
