---
id: "DESIGN-034RBzdpo91wcbqBK7Gb9t"
type: "design"
status: "approved"
title: "SYN-OBS-01 Design — Host所有 Logging Owner・設定解決・Source Catalog"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-01-desktop-app-bootstrap-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md"
    - "docs/adr/syn-obs-01-crate-ruviform-observability-logging-feature-host-global-subscriber.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/276"
    - "https://github.com/scarecrow173/ruviform/issues/277"
    - "https://github.com/scarecrow173/ruviform/issues/289"
  defers: []
  deferred-by: []
---

# SYN-OBS-01 Design — Host所有 Logging Owner・設定解決・Source Catalog

## Context

`synvelis-app` は SV-GUI-01 で確立した composition root(headless lib +
`webview` feature bin)を持ち、`ObservabilityOwner` が `syn.app.info` 向け
カウンタを保持済み。本設計はここに `ruviform-observability` の
`logging` feature(#276)と OBS-02 Source 語彙(#277)を接続し、
Issue #81 の「Host 一元所有・Global subscriber は Process 1 回・
library 側から全局登録しない」を構造で担保する。

責務分割:

- **Ruviform 側**(変えない): sink 構築・EnvFilter・bounded writer・
  WriterGuard・LoggingHealth・Source 契約型。
- **Synvelis 側**(本設計): 製品設定語彙→`LoggingConfig` 翻訳、
  process entrypoint での install、guard/health の保持、
  `syn.app.info` への health 反映、Synvelis 側カウンタの
  `CounterSource` 記述。

## Scope

### In Scope

- `crates/synvelis-app/src/logging.rs` 新設(headless、webview 非依存。
  `clippy::module_name_repetitions` 回避のため公開型名は module 名
  `logging` と prefix/suffix が一致しない `LogSettings`/`HostLog`/
  `LogInstallError` とする):
  - `LogSettings` — 製品設定(env var + CLI arg 由来の入力を保持)。
  - `CliOverrides` / `EnvOverrides` — bin が argv/env から集めた
    `Option` 値。env var の**読み取り自体は bin(process 境界)のみ**
    が行い、lib は純粋関数 `LogSettings::resolve(cli, env)` で
    `CLI arg > env > 既定(OFF)` を合成する(`settings_from_env` の
    ような lib 側 env アクセスは作らない)。`parse_args(&[String])
    -> CliOverrides` は純粋関数として lib に置く。
  - `to_logging_config(&LogSettings) -> Option<LoggingConfig>` —
    OFF なら `None`。
  - `HostLog` — install 状態・`WriterGuard` 群・`LoggingHealth`・
    `degraded` を保持する owner。
  - `install(&LogSettings) -> Result<HostLog, LogInstallError>`
    — `build_logging` → `set_global_default`。
  - `LogFormat`/`RotationPeriod`/`SinkDestination` 等は ruviform 側
    canonical 型をそのまま再参照する(独自変換型を作らない)。
- `ObservabilityOwner` 拡張: `LoggingHealth` の attach、Source catalog
  (`MetricSource`/`CounterSource`)の保持、`syn.app.info` への
  `logging` セクション追加。
- `bin/synvelis.rs`: 起動引数 parse → install → `SynvelisApp` へ
  health 接続 → `run()` 終了後の順序付き drop。
- `service.rs`/`app.rs` の非 RT 箇所への限定 `tracing` イベント発行。
- テスト: `tests/logging.rs` 新設(設定解決・OFF 既定・build→scoped
  dispatch・health 参照・catalog 整合)。

### Out of Scope

- Collector/registry/Snapshot 履歴(#289)。`ObservationRegistrar`
  実装を Synvelis 側に作らない。
- Diagnostics/Debug/Host Runner(#291/#292/#293)、`tracing_export`(#278)。
- Window 単位 Subscriber・GUI 購読 Adapter・Diagnostics Panel。
- RT callback での `tracing`/alloc/lock(既存 RT contract 維持)。
- PoC bin(`recording-poc`/`mcp-poc`/`gw10_report`)への install。

## Components and Boundaries

### `logging` module(新設、headless)

```
LogSettings            // 製品設定(素の値、ruviform 型を漏らさない)
  enabled: bool            // 既定 false
  filter: String           // EnvFilter 構文、ON 時既定 "info"
  stderr_format: LogFormat // Text 既定 / JsonLines
  file_dir: Option<PathBuf>// RollingFile sink、Some なら file sink 追加
  rotation: RotationPeriod // Daily 既定
  max_files: NonZeroUsize  // 既定 14
  queue_capacity: NonZeroUsize // 既定 8192(file sink)

HostLog
  state: InstallState      // Off | Installed | AlreadyInstalled
  guards: Vec<WriterGuard> // sinks 順、drop で flush/close
  health: Option<LoggingHealth>
  degraded: Vec<SinkDegraded>

InstallState               // Off | Installed | AlreadyInstalled
LogInstallError        // Build(LoggingBuildError)
```

`install()` の手順:

1. `settings.enabled == false` → `state: Off`、何も構築しない。
2. `to_logging_config` → `build_logging(&config)`。失敗は
   `LogInstallError::Build`(`Required` sink 失敗・全 sink 失敗・
   filter パース失敗を含む)。
3. `tracing::dispatcher::set_global_default(dispatch)`:
   - 成功 → `state: Installed`、guards/health/degraded を保持。
   - 失敗(既に global default あり) → **エラーにしない**。
     `state: AlreadyInstalled` とし、構築した guards は即 drop
     (=sink を正常停止)して既存 subscriber を侵さない。
     `status()` で明示報告する。

`AlreadyInstalled` を非エラーにする理由: headless テストや将来の
multi-entrypoint で install を冪等に呼べる方が事故らない。失われるのは
「その呼出が建てた sink」のみで、既存 subscriber とプロセス動作は
無傷。`status()`/`app_info` で必ず可視化する。

### `ObservabilityOwner` 拡張

```rust
pub struct ObservabilityOwner {
    intents: Counter,
    intent_errors: Counter,
    sources: Vec<MetricSource>,       // OBS-02 canonical
    logging_health: Option<LoggingHealth>,
}
```

- `new()` で `intents`/`intent_errors` を `counter()` で生成し、
  対応する `CounterSource`(`descriptor: CounterDescriptor { key:
  "synvelis.app.intents" / "synvelis.app.intent_errors", unit: Count,
  reporting: SnapshotOnly }`、`scope: ObservationScope::Process`、
  `reader: counter_reader`)を `MetricSource::from` で catalog 化。
- `attach_logging(&LoggingHealth)` — install 済み health を接続。
- `sources() -> &[MetricSource]` — 将来 `ObservationRegistrar` への
  登録対象(#289 で接続)。
- `logging_snapshot() -> Option<HealthSnapshot>`。

Source catalog は**記述と保持だけ**。登録先(registrar)は存在しない
ため、reader cell の所有は owner 内に留まる。`MetricSource` は
`reader()` で `&MetricReader` を返すので、`syn.app.info` は従来通り
reader から直接 read する。

### `syn.app.info` 拡張

```jsonc
"logging": {
  "state": "off" | "installed" | "already-installed" | "unavailable",
  "sinks": [ { "kind": "stderr"|"rolling-file", "state": "active"|"failed",
               "events_written": n, "dropped_queue_full": n,
               "partial_writes": n, "write_failures": n,
               "flush_failures": n, "channel_disconnected": n,
               "shutdown_abandoned": bool } ],
  "degraded": [ { "sink_index": n, "kind": "...", "reason": "..." } ]
}
```

`state` は `ObservabilityOwner` が知る範囲: health 未接続→`off`
(または未 install)、接続済→`installed`。`already-installed` は
`HostLog` が app へ通知する形にするため、`attach_logging` は
`(state, Option<LoggingHealth>)` を受け取る。

### `bin/synvelis.rs` startup/shutdown

```
main():
  parse_args(argv) -> CliOverrides    // lib の純粋関数
  EnvOverrides::read()                // bin 内の env var 読み取り
    --log[=FILTER]        // 有効化(省略時 filter="info")
    --log-file <DIR>      // RollingFile sink 追加(有効化含む)
    --log-format text|jsonl
    env: SYNVELIS_LOG(EnvFilter 構文、空でなければ有効)
         SYNVELIS_LOG_FILE, SYNVELIS_LOG_FORMAT
  LogSettings::resolve(cli, env)      // lib の純粋関数
install(&settings) -> HostLog        // main 先頭で1回
app.attach_logging(state, health)
... run() ...
run 終了 → app drop → HostLog drop で guards 順に flush/close
```

停止順序の未実装段(Audio Quiescence→Collector snapshot→Export
drain→Guard flush)のうち、現段階で存在するのは Guard flush のみ。
`HostLog` は drop 順序をフィールド順で保証し、
`WriterGuard` 側の bounded-timeout join に委譲する。
**Global subscriber は install 後に解除できない**(tracing 構造制約)
ことを doc-comment と本設計書に明記する。

### tracing イベント発行点(非 RT のみ)

- `bin/synvelis.rs`: `info!` 起動・backend probe 結果・asset bundle
  結果・終了。既存 `eprintln!` はユーザ向け即時出力として維持し、
  tracing は並行して発行する(sink ON 時のみ可視)。
- `service.rs`/`app.rs`: op 失敗時 `warn!`、project new/open/save
  成功 `info!`、op 受領 `debug!`(record_intent 発火点)。
- 機密分類: event field にパス・秘密情報を書かない
  (`project_new` の path は field 化しない — adapter は内容を
  検査しないため emit 側規約で防ぐ)。

## Data and Control Flow

- Startup: argv/env → `LogSettings` → `install` →(ON 時)
  `build_logging` → `Dispatch` → `set_global_default` → `HostLog`
  保持 → `SynvelisApp` へ `(state, health)` 接続 → UiRuntime/Service。
- Runtime: 非 RT イベント → global subscriber → per-sink EnvFilter →
  fmt(text/jsonl) → bounded queue → background worker → stderr/file。
  Metrics は `ObservabilityOwner` の counter/reader が独立して動作
  (Logging OFF でも読める)。
- Query: `syn.app.info` → `logging_snapshot()` → HealthSnapshot を
  JSON 化(非 RT・有界・その場限り)。
- Shutdown: `run()` 戻り → app/service drop → `HostLog` drop →
  各 `WriterGuard` が worker drain+flush+bounded join。

## Non-Functional Constraints

- RT: RT crate/`rt_` レーンへ `tracing` を導入しない。`tracing` は
  `synvelis-app` のみの直接依存とし、`cargo metadata` で他 crate の
  依存木に入らないことを検証する。
- 信頼性: `Required` stderr sink 失敗は install 失敗(明示エラー)、
  file sink は `BestEffort`(失敗は `degraded` 記録で動作継続)。
- セキュリティ: log event に認証情報・プロンプト・生 PCM・
  ファイルパスを書かない emit 側規約。
- 後方互換: `SynvelisApp::new` シグネチャ不変、logging 未接続でも
  既存テスト全てが動く。
- 性能: OFF 時は subscriber 未構築で tracing イベントは no-op 分岐
  のみ。ON 時も emit は非 RT 箇所のみ。

## Risks and Trade-offs

- `AlreadyInstalled` を非エラー化 → 「期待した sink に届いていない」
  状態を気付きにくい → `status()`/`app_info`/`degraded` で必ず
  可視化し、テストで明示する。
- `set_global_default` 失敗時に構築済み sink を drop → 起動直後に
  二重 install された場合、その間のイベントは既存 subscriber に
  届く。許容(プロセスで1回の運用が前提)。
- `--log` なしで file sink だけ指定 → file 指定は有効化を含意
  (利便性)。OFF 明指定の env/flag は設けない — 既定が OFF なので。
- EnvFilter パース失敗 → `build_logging` の `InvalidFilter` で
  install 失敗 → bin はエラーを stderr に出し exit code 4
  (設定不備)で終了。静かに無効化しない。

## Deferred Design Concerns

- `ObservationRegistrar` への catalog 登録 — #289 マージ時に
  `ObservabilityOwner` から registrar へ sources を渡す接続を追加。
- Host Runner(#293)による startup/stop 統合 — 現状の手順 install +
  drop 順を Runner 契約へ移行。
- GUI 購読(Adapter/Subscription/Permission/Redaction)— SV-GUI-05 系。
- Collector final snapshot・Export drain の停止順序 — #289/#278 後。
- tracing event の Correlation/Trace context — #278 OBS-03 後。

## Verification Notes

- [ ] `LogSettings` 解決: CLI>env>既定、OFF 既定、`--log` 裸で
  `info` filter、不正 filter で install 失敗。
- [ ] `install` → `with_default(&dispatch)` スコープ内で event が
  sink に届くこと(global 登録しない検証は scoped dispatch で代替)。
- [ ] `AlreadyInstalled`: 2 回目 install が既存 subscriber を壊さず
  `already-installed` 状態になる。
- [ ] `syn.app.info` の `logging` セクションが OFF/ON で正しく
  変わり、health counters が見える。
- [ ] Source catalog が `MetricSource` で構成され descriptor/reader
  が counter と一致。
- [ ] `mise run check/lint/test/test-rt/check-headless-deps` pass。
