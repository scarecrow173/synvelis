---
id: "ADR-034RBxHj0wzQculGmgXguq"
type: "adr"
status: "accepted"
title: "SYN-OBS-01 単一crate ruviform-observability logging feature 採用と Host 所有 Global Subscriber"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/81"
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
    - "docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md"
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/275"
    - "https://github.com/scarecrow173/ruviform/issues/276"
    - "https://github.com/scarecrow173/ruviform/issues/277"
    - "https://github.com/scarecrow173/ruviform/issues/289"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# SYN-OBS-01 単一crate ruviform-observability logging feature 採用と Host 所有 Global Subscriber

## Context and Problem Statement

Synvelis の Rust Host に観測基盤を導入する必要がある(Issue #81)。
Ruviform 側は `ruviform-observability` **単一クレート**を正本とし、
OBS-01(#276)の汎用 Logging Adapter(`logging` feature、opt-in)と
OBS-02(#277)の Source 登録契約が main に存在する。Collector(#289)/
Diagnostics(#291)/Debug(#292)/Host Runner(#293)は upstream 未提供。

問いは2つ:

1. Logging/観測の導入単位をどうするか(単一 crate feature / 別 crate /
   独自実装)。
2. Global subscriber・WriterGuard・設定解決・Source 記述の所有者を
   どこに置くか(library 側 / Host 側)。

## Decision Drivers

* Issue #81 が「`ruviform-observability` 単一クレート採用・
  `ruviform-observability-logging` 等の別クレート不使用・
  Ruviform library 側から全局登録しない」を明示している。
* Multiwindow/Headless で global 二重登録・不明な Guard Drop・
  停止競合を起こさない所有構造が必要。
* 製品判断として Logging 既定値・設定語彙は Synvelis が決める
  (AudioLens `AL_LOG` 語彙をコピーしない)。
* 汎用機構は Ruviform 責務、製品固有は Synvelis 責務の分割を守る
  (重複実装・恒久回避策を作らない)。
* upstream 未提供の Collector に対して Synvelis 側で代替 registry を
  作ると #289 と責務が衝突する。

## Considered Options

* **Option A**: `ruviform-observability` 単一クレートの `logging`
  feature を `synvelis-app` で有効化し、Host 側 owner 型が設定解決・
  `Dispatch` install・WriterGuard/Health 保持を行う。Source は
  OBS-02 canonical 型で記述・保持し、registrar 実装は upstream
  #289 待ち。
* **Option B**: `ruviform-observability-logging` / `-collector` 等の
  分離クレートが将来出た場合に備えた抽象化を先に作る。
* **Option C**: Synvelis 独自のロギング/簡易 Collector を内製し、
  Ruviform 側提供を待たずに機能を先行させる。

## Decision Outcome

Chosen option: **Option A**。

- `synvelis-app` の `ruviform-observability` 依存に `logging` feature
  を有効化する。別名分割クレートは導入しない(Issue #81 明記)。
- Global subscriber は **Host(process entrypoint)のみ**が
  `tracing::dispatcher::set_global_default` で install する。
  `synvelis-app` の lib/`SynvelisApp` は `Dispatch` を自分で全局登録
  しない(テストで複数 `SynvelisApp` を同一プロセスに作れるよう
  install は entrypoint 側の1回操作とする)。
- 設定解決・ON/OFF 既定値・sink 構成は Synvelis 製品語彙で Host が
  決定し、`LoggingConfig` に翻訳して `build_logging` に渡す。
  `SYNVELIS_LOG` / `--log` 語彙を採用し `AL_LOG` はコピーしない。
- WriterGuard/`LoggingHealth`/`degraded` 情報は Host 所有の owner
  型がプロセス寿命まで保持する。Window close で共有基盤を止めない。
- Synvelis 側カウンタは OBS-02 の `MetricSource`/`CounterSource`/
  `MetricDescriptor`/`ObservationScope` で記述する Source catalog を
  `ObservabilityOwner` に保持する。**registrar/collector 実装は
  Synvelis 側に作らない**(#289 の upstream 責務)。
- Counter→Log/Severity/Delta 変換は作らない。

### Consequences

* Good, because Issue #81 の「単一クレート・Host 一元所有・library
  非全局登録」をそのまま満たし、Multiwindow 時も install は
  プロセス 1 回に固定される。
* Good, because OFF 経路では subscriber を構築しないため、既定の
  GUI 利用に tracing スタックのコストとノイズが載らない。
* Good, because Source catalog が canonical 型のみで記述されるため、
  #289 マージ時に登録先を差し替えるだけでよい。
* Bad, because Collector 未到着の間、Counter/Gauge の
  registry 経由 Snapshot は取れず `ObservabilityOwner` 直接参照に
  留まる(受入条件の Collector 関連項は #289 依存として分離)。
* Bad, because Global subscriber は install 後に解除できない
  (tracing の構造制約)。OFF→ON の実行中切替は再起動を要する。

### Confirmation

- `mise run check`/`lint`/`test`/`test-rt` が pass。
- `crates/synvelis-app` 以外への `tracing` 依存追加なし、RT 対象
  crate の依存木に `tracing` が入らないことを `check-headless-deps`
  と Cargo メタで確認。
- spec Acceptance Criteria の検証は `docs/test-specs` の SYN-OBS-01
  検証仕様で担保する。

## Implementation Plan

* Affected paths: `Cargo.toml`(workspace)、`crates/synvelis-app/Cargo.toml`、
  `crates/synvelis-app/src/`(logging owner モジュール新設・`app.rs`・
  `service.rs`・`bin/synvelis.rs`)、`crates/synvelis-app/tests/`。
* Patterns to follow: SV-GUI-01 の「headless lib + webview feature bin」
  分離。owner 型は lib 側・install 呼出は bin 側。
* Patterns to avoid: lib からの `set_global_default`、独自 severity/
  delta 変換、環境変数を library 側で読む設計、PoC bin への logging
  組み込み。
* Dependencies or configuration: `ruviform-observability` git 依存
  (`logging` feature)、`tracing = "0.1"`(workspace deps)。
  `SYNVELIS_LOG`/`SYNVELIS_LOG_FILE`/`SYNVELIS_LOG_FORMAT`・
  `--log`/`--log-file`/`--log-format`。

## Verification

* [ ] `mise run check` / `mise run lint` / `mise run test` pass
* [ ] `mise run test-rt` pass(RT 非計装の維持)
* [ ] `mise run check-headless-deps` pass
* [ ] 二重 install 抑止・OFF 既定・health 参照の headless テスト追加

## Pros and Cons of the Options

### Option A

* Good, because upstream 契約と責務境界をそのまま使い、将来の
  #289/#291/#293 接続点が Source catalog と owner 型に限定される。
* Good, because 追加実装量が最小で、既存 `ObservabilityOwner`/
  `syn.app.info` に自然に載る。
* Bad, because Collector 系の受入は upstream 待ちで本 Issue 単独では
  完結しない(段階導入の性格上やむを得ない)。

### Option B

* Good, because 将来の crate 分割に備えた抽象化が先行する。
* Bad, because Issue #81 が別クレート不採用を明記しており、
  仮想要件への YAGNI 違反の抽象化になる。

### Option C

* Good, because Collector 相当機能を upstream を待たず使える。
* Bad, because #289/#291/#293 の責務を Synvelis 側に恒久重複実装
  することになり、AGENTS.md のマルチリポジトリ運用規約
  (「Ruviform の問題を Synvelis 側の恒久的な重複実装で隠さない」)
  に反する。

## More Information

- 正本 spec: `docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md`
- upstream: [Ruviform #275](https://github.com/scarecrow173/ruviform/issues/275) /
  [#276](https://github.com/scarecrow173/ruviform/issues/276) /
  [#277](https://github.com/scarecrow173/ruviform/issues/277)
