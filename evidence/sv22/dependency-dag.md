# SV22 Dependency DAG Evidence — TA-001 / SV22-C05

対象 Revision:

- Synvelis: branch `scarecrow173/sv22-target-architecture`、
  worktree HEAD `7802ac7ae3e88eac1e67f8b734b68dca22bd2e7f`
  （docs 追加前の base。PR 時点で merge 後 SHA を別途記録）
- Ruviform: `Cargo.lock` resolved
  `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`
  （`mise run check-ruviform-rev` = Pass、27 lockfile sources
  が単一 SHA に一致）
- Ruviform checkout（ソース参照）:
  `~/.cargo/git/checkouts/ruviform-8f0ffdb08acc57df/fd1d4d5`

## 実行環境

- OS: Windows（MINGW64_NT-10.0-26300, Git Bash 3.6.7）
- rustc/cargo: 1.99.1（mise 管理 toolchain。MSRV 1.89 /
  CI pin 1.99.0 — `mise.toml`/`mise.ci.toml` 参照）
- node: v26.12.1（`tools/check-*.mjs` および DAG 集計に使用）
- cargo metadata: `--format-version 1 --locked --offline`
  （`Cargo.lock` を変更しない読み取り専用）

## 再現コマンド

```sh
# 1. resolve グラフ取得（locked・offline・読み取り専用）
cargo metadata --format-version 1 --locked --offline > /tmp/sv22-meta.json

# 2. cycle 検査 + 辺一覧（node ワンライナー。dep_kinds.kind=="normal"
#    のみを辺とし DFS で cycle 検出。dev/build/optional/feature/
#    target-specific は別分類）
node -e '<resolve.nodes を走査し normal 辺で DFS cycle 検査>'
```

`cargo metadata` の resolve は feature を含まない全依存宣言を
解決するため、optional dep は node として存在するが、feature
未有効でも辺として観測される。本台帳では `dep_kinds.kind` が
`normal` の辺を「通常依存」、`dev`/`build` をそれぞれ
「dev-dep」「build-dep」として分類し、optional は
`Cargo.toml` の `optional = true` と feature 対応で別記する。

## 結果サマリ

| 検査 | 範囲 | 結果 |
| --- | --- | --- |
| normal-dep cycle | Synvelis resolve 全体（Synvelis 12 member + resolved ruviform 24 package + 外部依存） | **0** |
| normal-dep cycle | Ruviform workspace 全体（fd1d4d5 checkout、42 member） | **0** |
| dev/build-dep cycle（参考） | 同上 | 0（dev-dep 循環は Cargo が許容するが、観測上も存在しない） |
| Synvelis→Ruviform 方向 | 全 synvelis crate | 一方向のみ。Ruviform のいずれの crate も synvelis package に依存しない |
| Ruviform→Consumer | ruviform-* 全 crate | synvelis/audio-lens package node なし |

## Synvelis workspace → 依存辺一覧（resolve 実測）

`()` 内は dep_kinds。`(dev)` は dev-dependency のみの辺、
`(/dev)` は normal+dev の混在辺（normal 辺としても存在）。

```
mcp-poc            -> synvelis-gateway(), synvelis-project()
recording-poc      -> synvelis-project(), synvelis-recording()
synvelis-app       -> ruviform-audio(), ruviform-audio-core(),
                      ruviform-audio-plugin-bridge(), ruviform-media-core(),
                      ruviform-media-reader(), ruviform-observability(),
                      ruviform-plugin-core(), ruviform-plugin-host(dev),
                      ruviform-ui(), ruviform-ui-test(dev),
                      synvelis-library(), synvelis-player(),
                      synvelis-project(), synvelis-recording(),
                      synvelis-workspace()
synvelis-gateway   -> synvelis-project()
synvelis-library   -> ruviform-io-file(), synvelis-project()
synvelis-player    -> ruviform-audio(), ruviform-audio-core(/dev),
                      ruviform-audio-plugin-bridge(), ruviform-exec(),
                      ruviform-graph(/dev), ruviform-io-file(dev),
                      ruviform-io-symphonia(), ruviform-io-wav(/dev),
                      ruviform-media-core(), ruviform-media-io(/dev),
                      ruviform-media-reader(), ruviform-plugin-core(),
                      ruviform-plugin-host(), ruviform-rt(),
                      synvelis-library(), synvelis-project(),
                      synvelis-recording()
synvelis-project   -> ruviform-audio-core(), ruviform-graph(),
                      ruviform-io-file(), ruviform-media-core(),
                      ruviform-plugin-core()
synvelis-recording -> ruviform-audio-core(), ruviform-io-file(),
                      ruviform-media-core(), ruviform-rt(),
                      synvelis-project()
synvelis-render    -> ruviform-audio(), ruviform-audio-core(),
                      ruviform-exec(), ruviform-graph(), ruviform-io-file(),
                      ruviform-io-symphonia(), ruviform-io-wav(),
                      ruviform-media-core(), ruviform-media-io(),
                      ruviform-media-reader(), ruviform-rt(),
                      synvelis-player(), synvelis-project(),
                      synvelis-recording(dev)
synvelis-theme     -> ruviform-ui-theme()
synvelis-workspace -> ruviform-media-core(), ruviform-plugin-core(),
                      ruviform-rt(), ruviform-ui(), ruviform-viz(),
                      synvelis-gateway(), synvelis-project()
```

## Optional / feature-gated / target-specific 分類

`synvelis-app` の GUI driver 依存は optional dep であり、
feature でのみ有効化される（`crates/synvelis-app/Cargo.toml`）:

| optional dep | feature | 備考 |
| --- | --- | --- |
| `ruviform-ui-webview` | `webview` | 旧 WebView driver（移行期間保持・新規本番採用ではない） |
| `ruviform-ui-gpui` | `native`（`gpui-test` は `native` を包含） | GPUI 標準 Driver。GPUI 型の進入は composition root のみ |
| `rust-embed` | `webview` | WebView 資産 embed |
| `synvelis-theme` | `native` | `ruviform-ui-theme` の consumer 側 theme crate |

これらは default resolve の通常辺には含まれない（feature 未
有効時）。`native`/`webview` 有効時も `synvelis-*` →
`ruviform-ui-*` の一方向のみで循環を生じない。

`synvelis-app` → `ruviform-plugin-host`・`ruviform-ui-test`
は **dev-dependency**（テスト専用）であり通常依存ではない。
`synvelis-player`/`synvelis-render` の一部 ruviform 辺は
normal+dev 混在で、normal 辺として存在する。

## Ruviform workspace 内辺一覧（fd1d4d5 resolve 実測・抜粋）

Synvelis が直接・推移的に依存する ruviform crate の通常依存:

```
ruviform-audio               -> ruviform-audio-analysis(), ruviform-audio-core(),
                                ruviform-exec(), ruviform-graph(), ruviform-io-file(),
                                ruviform-media-core(), ruviform-media-io(),
                                ruviform-observability(), ruviform-rt()
ruviform-audio-analysis      -> ruviform-audio-core(), ruviform-dsp(), ruviform-media-core()
ruviform-audio-core          -> ruviform-media-core()
ruviform-audio-plugin-bridge -> ruviform-audio(), ruviform-audio-core(), ruviform-graph(),
                                ruviform-media-core(), ruviform-plugin-core(),
                                ruviform-plugin-host(), ruviform-rt()
ruviform-dsp                 -> ruviform-audio-core(), ruviform-math(), ruviform-media-core()
ruviform-exec                -> ruviform-rt()
ruviform-graph               -> ruviform-audio-core()
ruviform-io-file             -> (なし)
ruviform-io-symphonia        -> ruviform-audio-core(), ruviform-media-core(), ruviform-media-io()
ruviform-io-wav              -> ruviform-audio-core(), ruviform-media-core(), ruviform-media-io()
ruviform-media-core          -> (なし)
ruviform-media-io            -> ruviform-audio-core(), ruviform-media-core(), ruviform-video-core()
ruviform-media-reader        -> ruviform-audio-core(), ruviform-media-core(), ruviform-media-io(),
                                ruviform-rt()
ruviform-observability       -> ruviform-media-core(), ruviform-rt()
ruviform-plugin-core         -> ruviform-audio-core(), ruviform-media-core(), ruviform-video-core()
ruviform-plugin-host         -> ruviform-audio-core(), ruviform-media-core(),
                                ruviform-observability(), ruviform-plugin-core(),
                                ruviform-rt(), ruviform-video-core()
ruviform-rt                  -> (なし)
ruviform-ui                  -> ruviform-exec(), ruviform-rt()
ruviform-ui-test             -> ruviform-ui()
ruviform-ui-theme            -> (なし)
ruviform-video-core          -> ruviform-media-core()
ruviform-viz                 -> ruviform-exec(), ruviform-media-core(), ruviform-numeric()
```

注: 上記は Synvelis resolve に現れた 24 ruviform package の
辺。Ruviform workspace 全体（42 member）でも normal-dep
cycle = 0 を別途確認済み（`crates/ruviform-plugin-host/
Cargo.toml` の dev-dep `ruviform-audio` は dev 辺として
分類）。

## TA-005 注目辺の分類

| 辺 | kind | 分類 | 評価 |
| --- | --- | --- | --- |
| `ruviform-plugin-host` → `ruviform-audio` | dev のみ | dev-dep（`tests/plugin_graph_e2e.rs` の AC3 契約） | 通常依存の循環ではない。host→engine 方向の不要依存なし |
| `ruviform-audio` → `ruviform-plugin-host` | なし | — | Engine→Host 依存なし |
| `ruviform-audio` → `ruviform-audio-plugin-bridge` | なし | — | 接着は bridge が正方向に担う |
| `ruviform-audio-plugin-bridge` → `ruviform-audio` + `ruviform-plugin-host` | normal | bridge→両者 | 設計どおりの接着層 |

## GUI driver 辺の分類

| 辺 | kind | 分類 |
| --- | --- | --- |
| `synvelis-app` → `ruviform-ui-gpui` | optional（`native` feature） | feature-gated。GPUI 型は composition root のみ |
| `synvelis-app` → `ruviform-ui-webview` | optional（`webview` feature） | feature-gated。旧経路・移行期間保持 |
| `synvelis-app` → `synvelis-theme` | optional（`native` feature） | feature-gated |
| `synvelis-theme` → `ruviform-ui-theme` | normal | consumer theme crate → driver-neutral theme 契約 |

## 結論

- Synvelis workspace + resolved Ruviform の **通常依存 DAG
  に循環なし**（再現可能コマンド・対象 Revision 記録済み）。
- Synvelis→Ruviform は一方向、Ruviform→Consumer 依存なし。
- plugin-host→audio は dev-dep のみで TA-005 の境界を満たす。
- GUI driver 依存は feature-gated で headless 契約を侵さない。
