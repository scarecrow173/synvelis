---
id: "EXP-154-mcp-poc-control-port-flake"
title: "Issue #154 mcp-poc real-transport ConnectionReset 計測・原因・修正"
created: "2026-11-04"
updated: "2026-11-04"
---

# Issue #154 mcp-poc real-transport ConnectionReset 計測・原因・修正(2026-11-04)

- Issue: https://github.com/scarecrow173/synvelis/issues/154
- 計測データソース: GitHub Actions API(`gh api repos/.../actions/runs|jobs|logs`)、
  `ci` workflow の直近 31 run(runNumber 1–31、全て 2026-11-04)
- 関連: ruviform #238(`l1_proxy_e2e` flake、別機構・#241 で解消済み)

## 1. 判定結論(要約)

- **閾値判定: 修正を実施。** 明示的な閾値は repo に未定義だが、直近 31 run で
  同一機構の ConnectionReset が **3 件** 確認された(下表)。required lane の
  fail-closed gate に対し run 全体を赤くする flake であり、原因が特定できて
  構造的に除去可能なため「計測のみ・監視継続」ではなく修正を正当と判断。
- 原因は **テスト harness のポート選択プロトコルの TOCTOU** であり、
  retry 増加・タイムアウト延長では隠せない構造的问题。
  `mcp-poc serve` 側が `--control-port 0` で bind し実ポートを stderr で
  報告する方式へ変更し、競合ウィンドウを構造的に除去した(§4–5)。

## 2. 実測: 直近 31 run の失敗履歴

msrv-verify の全 leg 結果を `gh api .../runs/<id>/jobs` で収集し、
失敗 leg は `.../jobs/<job_id>/logs` で原因を個別特定した。
verify/gw10-verify についても同一シグネチャの失敗を横断確認した。

| run | event/branch | 失敗 lane | 失敗テスト | panic 箇所 | 分類 |
| --- | --- | --- | --- | --- | --- |
| [41815380858](https://github.com/scarecrow173/synvelis/actions/runs/41815380858) | PR #153 (syn-151) | msrv-verify | `stdio_e2e::delegated_apply_commits_retries_without_duplicates_and_survives_restart` | `common/mod.rs:50` wire write `ConnectionReset`(code 104) | **本 flake** — attempt 2 rerun で pass |
| [41816354694](https://github.com/scarecrow173/synvelis/actions/runs/41816354694) | push main (16e10c6) | verify (ubuntu) | `gw10_transport::gw10f_delegated_apply_commits_and_survives_restart_real` | `Connection reset by peer` | **本 flake**(issue-147 exp でも記録済み) |
| [41768184551](https://github.com/scarecrow173/synvelis/actions/runs/41768184551) | push main | msrv-verify | `gw10_transport::gw10f_delegated_apply_commits_and_survives_restart_real` | `common/mod.rs:57` wire read `ConnectionReset`(code 104) | **本 flake** |
| 41765834163 | PR syn-83 | 全 lane | — | `rust@1.99.0 is not in the lockfile`(mise) | CI bring-up 時の lockfile 問題、本件と無関係 |

- run 単位: **3/31 run ≈ 9.7%** がこの flake で失敗。
- delegated real-transport テスト実行単位: 1 run あたり verify×3OS
  (各 job が stdio_e2e+gw10_transport の両 binary を実行=2 実行)×3 +
  msrv×2 + gw10-verify×3OS(各 1 実行)= 概ね 11 実行/run、計 ~340 実行。
  **3/~340 ≈ 0.9%/実行**。
- 失敗 lane は msrv-verify×2・verify(ubuntu)×1 で lane 固有ではなく、
  同一テスト binary を動かす全 lane に発生し得る(標本 3 件での偏りは
  確率変動の範囲)。
- 3 件全てが **delegated-apply 系テスト**(`..._survives_restart`)に集中。
  同テストは control channel(TCP)RPC を最も多く実行し、
  hijack された接続が RST を受ける露出ウィンドウが最大(§3)。

## 3. 原因分析

### 症状の特定

- 2 件は `common/mod.rs:50`(`send_raw` の `write_all`)と
  `common/mod.rs:57`(`read_line` の `read_line`)の TCP 側 panic。
  Linux で stdio pipe への write は EPIPE(code 32)、read は EOF を返すため、
  **code 104 `ConnectionReset` は TCP control channel 上でのみ発生し得る**。
  stdio 経路では絶対に出ないシグネチャ。

### 機構(probe→bind TOCTOU)

旧 `Server::spawn` は `free_port()`(= `TcpListener::bind(127.0.0.1, 0)` で
空きポートを取得して即 drop)でポートを「予約」し、それを
`mcp-poc serve --control-port <P>` へ渡していた:

1. test A が `free_port()` で P を probe → drop(P は再び空き)
2. 並列テスト B が `free_port()` → 同じ P を取得する余地あり
   (probe drop 〜 child bind の ~10–100ms の boot ウィンドウ)
3. A・B 両 child が P を bind 競争 → 一方が `control_listener` の
   silent `return`(旧 `let Ok(listener) = ... else return`)で bind 失敗
4. 敗者テストの `control()` は `TcpStream::connect(P)` に成功するが、
   それは **勝者テストのサーバ** (または OS が割当てた他 process) —
   外部 listener への接続になる
5. 勝者のテストが終了して child が kill されると、乗っ取られた側の
   確立済み TCP 接続に RST → 次の wire read/write が `ConnectionReset`

- 確立済み接続への RST は未読データを残した socket close / process kill
  で発生し、`send_raw`(write RST)・`read_line`(read RST)両パターンを
  整合的に説明する。
- ruviform #238(`l1_proxy_e2e`)は別機構(shared-memory plugin 試験の
  wall-clock sleep 同期、Windows CI のスケジューリング遅延)で、
  #241(`f6d6936`)の pipeline barrier 化で解決済み。「実 process+実
  transport の timing 依存を明示的同期へ置き換える」という教訓の共有で
  あり、本件の直接原因ではない。

### 構造的問題としての整理

- `free_port()` probe は「port が空いていた過去時点」の情報でしかなく、
  child が bind するまでの間に並列テスト・runner 内の任意 process が
  同じ port を取得できる = 選択プロトコル自体が race を内包。
- `control_listener` の bind 失敗が silent `return` で、「自分の
  listener が無い」ことすらテスト側から区別不能だった。
- この類の flake は retry を増やしても「正しいサーバへの接続」という
  不変条件を回復できない(再接続は別の外部 listener に着き得る)。
  修正は port 選択プロトコルの境界で行う必要がある。

## 4. 修正内容

`mcp-poc serve` 側が listener を bind し、実ポートを readiness 信号として
stderr に報告する方式へ変更(racy な probe を構造的に除去):

- `crates/mcp-poc/src/main.rs`
  - `cmd_serve`: control listener を main thread で
    `TcpListener::bind(("127.0.0.1", port))` — `--control-port 0` で
    kernel が原子的に空き port を選択(probe/bind 間の競合が存在しない)。
  - bind 成功後 `eprintln!("control-port={bound}")` で実 port を報告。
  - bind 失敗は `fail()`(exit 1 + stderr message)へ — 旧 silent `return`
    (serve が制御面なしで動き続ける潜在的不具合)を fail-closed 化。
- `crates/mcp-poc/tests/common/mod.rs`
  - `Server::spawn`: `--control-port 0` 固定、stderr を piped 化し、
    drainer thread が `control-port=<N>` 行を拾う(30s 上限、
    未報告は child kill + stderr 末尾を含む panic = 正直な失敗)。
    drainer は EOF まで drain し続け、session thread の panic 出力で
    pipe が満杯になる wedge を防ぐ。
  - `Server::control()`: 報告済みの bound port へ直接 connect。
    「listener 未 bind」という中途状態が消えたため retry loop は除去
    (失敗は実欠陥として即 panic)。
  - `free_port()` 削除 — ポート選択の責務が server 側へ移ったため。

検証済み: `cargo test -p mcp-poc` 全 9 テスト pass、
`cargo clippy -p mcp-poc --all-targets -D warnings` / `cargo fmt --check` clean、
占有済み port への `serve` が `error: control listener bind failed`(exit 1)
を返すこと、`--control-port 0` が `control-port=<N>` を stderr 報告することを
実測確認(本 doc §6)。

## 5. 不変条件の回復

- 「`control()` は自 child の listener にのみ接続する」が構造的に保証:
  port は kernel が bind 時点で割当、報告は child 自身の stderr 経由で
  他テストの process からは見えない。
- 「bind 失敗が検出可能」: 未報告は 30s で明示 panic(旧: 他 listener への
  誤接続として不定形の失敗)。
- 検証の弱体化なし: テスト数・assertion・real-transport 経路は不変。
  `control()` から retry を除いたことでかえって厳格化。

## 6. ローカル検証記録

| 確認 | 結果 |
| --- | --- |
| `cargo test -p mcp-poc` | 9/9 pass(stdio_e2e 5 + gw10_transport 4) |
| `cargo clippy -p mcp-poc --all-targets -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |
| 占有済 port へ `serve --control-port <P>` | `error: control listener bind failed ... (os error 10048)` exit 1(旧: silent) |
| `serve --control-port 0` | stderr `control-port=53217` 報告確認 |

## 7. Shadow CI 証跡(issue-148 selective 判定の偽陰性対照)

本 PR の変更集合は `crates/mcp-poc/**`(gateway/ipc class、dep 伝播で
synvelis-workspace/synvelis-app も affected)+ `docs/**`。selective 意味論で
期待 lane は `classify`/`dep-guard`/`verify`/`msrv-verify`/`gw10-verify`、
not-applicable は `rt-verify`/`ui-verify`(×3OS)と予測される。
PR run の `classify` job 出力・evidence gate の shadow comparison
(would-skip leg の実結果)をここに記録する。

- PR: (created 後に記録)
- classify decision: (run 完了後に記録)
- shadow legs 実結果: (run 完了後に記録)

## 8. 残存リスク・監視

- `--control-port 0` 移行後は port 競合の経路が無いため、同シグネチャの
  再発は別原因を示す。#327 baseline の retry 統計で継続追跡対象。
- `foreign_server`(同一 dir への 2 台目 serve)は duress 下の動作を
  明示していないが、本件の失敗とは無関係であり現状維持。
- 30s の readiness 上限は loaded runner の余裕値であり、超過時は
  child stderr 末尾を含む正直な panic になる(隠蔽ではない)。
