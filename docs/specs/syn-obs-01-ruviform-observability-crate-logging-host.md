---
id: "SPEC-034RBvYIlw0wiBoGsL9EBY"
type: "spec"
status: "proposed"
title: "SYN-OBS-01 Ruviform observability 単一crate Logging/Host統合"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/81"
    - "https://github.com/scarecrow173/synvelis/issues/23"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/overview.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/275"
    - "https://github.com/scarecrow173/ruviform/issues/276"
    - "https://github.com/scarecrow173/ruviform/issues/277"
    - "https://github.com/scarecrow173/ruviform/issues/289"
    - "https://github.com/scarecrow173/ruviform/issues/278"
    - "https://github.com/scarecrow173/ruviform/issues/293"
    - "https://github.com/scarecrow173/ruviform/issues/281"
  defers: []
  deferred-by: []
---

# SYN-OBS-01 Ruviform observability 単一crate Logging/Host統合

## What Should Be Built

Synvelis の Rust Host(`synvelis-app` composition root とその process
entrypoint)が、`ruviform-observability` **単一クレート**の `logging`
feature(Ruviform #276 OBS-01)を使ってプロセス共通の Logging 基盤を
所有・導入する。Global subscriber の install、WriterGuard の寿命、
`LoggingHealth` の参照を Host が一元管理し、Synvelis 側の非 RT
イベントと将来 Ruviform 側が発行する非 RT `tracing` イベントを同一の
Subscriber に統合する。

あわせて OBS-02(#277)の Source 語彙(`MetricDescriptor` / `MetricSource` /
`CounterSource` / `ObservationScope` / `SourceId` + `SourceGeneration`)を
Synvelis 側 Counter の記述・保持に採用し、将来の Collector(#289 OBS-07)
登録にそのまま渡せる Source catalog を `ObservabilityOwner` に持つ。

## Why Now

- Issue #81 は Ruviform Observability 統合設計(#275)の Synvelis 側導入
  Issue であり、Logging Adapter は**最初から導入必須**と明記されている。
  Collector/Diagnostics/Debug/Host Runner は段階追加。
- SV-GUI-01(#110)で `synvelis-app` composition root と
  `ObservabilityOwner` スロットが既に存在し、GUI から
  `syn.app.info` でカウンタを参照できる。Logging 基盤の所有者を今
  決めておかないと、Multiwindow 化(SV-GUI-02)や Gateway/Player 実装の
  各所で独自の出力手段が増え、Global subscriber 二重登録・Guard Drop
  不明化・停止競合の構造問題を後から剥がすことになる。
- Ruviform #276/#277 は main マージ済み、#289/#291/#292/#293 は
  upstream で未完了。段階導入の第 1 段として Logging + Source 語彙
  採用を確定させるタイミング。

## Users and Value

- **Synvelis 開発者**: `--log` / `SYNVELIS_LOG` で headless・GUI・CLI を
  同一手順で観測でき、stderr/file の Text/JSONL・Filter・rotation を
  製品語彙で制御できる。
- **運用・サポート**: Sink の queue-full / WriteZero / IO 失敗 /
  shutdown abandon を `LoggingHealth` で非 RT に確認でき、ログ基盤
  自身の故障が黙って失われない。
- **エンドユーザ**: 既定 OFF で GUI 利用に stderr ノイズを出さず、
  ON にしても Playback/Project/Gateway の正本動作を阻害しない。

## Scope

### In Scope

- `synvelis-app` の `ruviform-observability` 依存に `logging` feature を
  有効化し、`tracing` 依存を追加する。
- Headless lib 内に Host 側 Logging 所有モジュールを新設する:
  - 製品設定(起動引数・環境変数)から `LoggingConfig` を解決する
    (Synvelis 語彙。AudioLens `AL_LOG` をコピーしない)。
  - `build_logging` → `Dispatch` → `set_global_default` の install
    経路(プロセス 1 回。二重 install は明示エラー状態として扱い、
    既存 subscriber を壊さない)。
  - `WriterGuard` / `LoggingHealth` / `degraded` sink 情報を保持する
    Owner 型。
  - Logging OFF 経路: subscriber を構築しない。Counter/Gauge 等の
    Metrics は OFF でも取得可能なままにする。
- `synvelis` bin(webview)の startup/shutdown への組み込み:
  startup 引数で Logging ON/OFF・filter・file sink を決定し、
  Guard をプロセス寿命まで保持する。
- `ObservabilityOwner` への `LoggingHealth` 接続と、
  `syn.app.info` 経由の Health Snapshot 参照(非 RT・有界)。
- Synvelis 側カウンタの `MetricSource`/`CounterSource` 化と
  Source catalog 保持(OBS-02 canonical 型のみ使用。
  Synvelis 独自の Delta/Severity/Trace 変換を作らない)。
- Headless 検証: install/設定解決/health 参照/二重 install 抑止の
  テスト(`cargo test`・`rt_` レーンが非計装であることの確認を含む)。

### Out of Scope

- Collector/Metrics 履歴/Snapshot 実装(Ruviform #289 OBS-07 待ち)。
  Synvelis 側で Collector 相当の重複実装・代替 registry は作らない。
- `tracing_export`(#278 OBS-03)、Diagnostics(#291)、Debug
  Inspection(#292)、Host Runner(#293)の統合 — すべて upstream
  提供後の段階追加。
- GUI Diagnostics Panel・Subscription/Refresh UI・Window 単位の購読
  管理(#114 SV-GUI-05 等)。Health Snapshot の IPC 公開は `syn.app.info`
  の情報追加に留める。
- RT Audio callback からの `tracing`/Logger/IPC/alloc/lock — 既存 RT
  Contract を変更しない(非計装を維持)。
- Gateway Audit/Grant/Provenance 永続正本との統合 — 分離を維持。
- JS `console` 自動 Bridge。
- `recording-poc`/`mcp-poc`/`gw10_report` 等の PoC・検証 bin への
  logging install(製品 Host のみが対象)。
- AudioLens `AL_LOG` 語彙の互換提供。

## Requirements

- Global subscriber install はプロセスにつき 1 回。
  Multiwindow/Headless で二重登録・不明な Guard Drop・停止競合を
  作らない。Global subscriber は install 後に解除できないことを
  ドキュメント上明記する。
- Ruviform library 側からの全局登録は行わない
  (`ruviform-observability` 側も `set_global_default` しない契約を
  そのまま利用する)。
- 設定解決は Host が所有する: ON/OFF、stderr/file、Text/JSONL、
  `EnvFilter` 構文 filter、rotation/retention/path、機密分類方針、
  既定値。`Logging OFF` の初期値は Synvelis の製品判断として
  **既定 OFF**(GUI エンドユーザにログを出さない。開発・診断は
  明示 opt-in)とする。
- Sink 構築失敗は `Required`/`BestEffort` を区別し、全 sink 失敗を
  成功と偽らない。`BestEffort` 失敗は `degraded` として可視化する。
- 停止順序: Audio/Plugin Writer の Quiescence(将来 Engine 接続時)→
  Collector 最終 Snapshot(#289 以降)→ Tracing Export Drain(#278 以降)
  → WriterGuard Flush/Close。現段階では WriterGuard の順序付き
  Drop(= flush)までを保証し、未実装段は契約として明記する。
- RT callback で `tracing`/Logger/IPC/alloc/lock を使わない。
  `rt_` テストレーンを含む既存 RT contract を維持する。
- `syn.*` サービス層・project/session/workspace 操作の失敗など
  非 RT イベントを `tracing` で発行し、Logging ON 時に sink へ
  届くこと。
- 機密分類: log event に秘密情報・認証情報・生 PCM・プロンプトを
  書かない方針を host 側規約として明記する(adapter は内容を
  検査しないため)。

## Acceptance Criteria

- [ ] `synvelis` 起動引数/環境変数で Logging ON/OFF・filter・
  stderr/file・Text/JSONL を制御でき、OFF 既定で subscriber が
  install されない。
- [ ] Logging ON 時、Synvelis 側非 RT イベントが設定された sink に
  到達し、同一 `Dispatch` が Global default としてプロセス全体で
  共有される(二重 install を試みても既存 subscriber が置き換わら
  ず、明示的に通知される)。
- [ ] `WriterGuard` が process 寿命まで保持され、正常終了時に
  flush/close される。`syn.app.info` から sink Health Snapshot
  (状態・drop/failure カウンタ)を参照できる。
- [ ] Synvelis 側カウンタが OBS-02 の `MetricSource`/`CounterSource`
  /descriptor で記述・保持され、Synvelis 独自の Counter→Log/
  Severity/Delta 変換が存在しない。
- [ ] `cargo check`/`lint`/`test`/`rt_` レーンが全て pass し、
  RT 対象 crate に `tracing` 依存・RT callback 内の tracing 呼出が
  無い。
- [ ] Logging OFF でも `ObservabilityOwner` のカウンタ取得・
  `syn.app.info` が動作する(Metrics は Logging と独立)。

## Deferred Design Concerns

- Collector/Metrics Snapshot/履歴 — Ruviform #289 (OBS-07) 提供後に
  Source catalog を `ObservationRegistrar` へ登録する経路を接続。
  再着手 trigger: #289 main マージ。
- Diagnostics/Health 集約(#291)、Debug Inspection(#292)、
  Host Runner(#293) による統合停止順序の自動化 — upstream 提供後。
- GUI 向け有界 Query/Subscription/Refresh/Permission/Redaction/
  Generation の Adapter — SV-GUI-05(#114)系で非 RT IPC として段階追加。
- Multiwindow での Window Subscriber 解除モデル — SV-GUI-02 以降。
  本 spec では process-global install のみを保証する。
- OBS-06(#281)Cross-Consumer Gate への実測提出 — 実装後の継続作業。

## Sources and Relations

- 正本 Issue: [Synvelis #81](https://github.com/scarecrow173/synvelis/issues/81)、
  横断依存正本 [Synvelis #23](https://github.com/scarecrow173/synvelis/issues/23)。
- upstream 提供物: [Ruviform #275 統合設計](https://github.com/scarecrow173/ruviform/issues/275)、
  [#276 OBS-01 Logging Adapter](https://github.com/scarecrow173/ruviform/issues/276)(merged)、
  [#277 OBS-02 Source 契約](https://github.com/scarecrow173/ruviform/issues/277)(merged)。
  未提供: #289 Collector・#278 tracing_export・#291・#292・#293(いずれも OPEN)。
- 依存: SV-GUI-01(#110)の `synvelis-app` composition root・
  `ObservabilityOwner`・`syn.*` service surface を前提とする。
