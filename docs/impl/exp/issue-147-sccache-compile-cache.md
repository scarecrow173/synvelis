---
id: "EXP-147-sccache-compile-cache"
title: "Issue #147 sccache コンパイルキャッシュ導入効果実測"
created: "2026-11-04"
updated: "2026-11-04"
---

# Issue #147 sccache コンパイルキャッシュ効果実測(2026-11-04)

- 計測契約: `docs/designs/issue-147-sccache-compile-cache.md` +
  ruviform CI-OPT-00 比較条件(OS/toolchain/profile/Cargo.lock/
  Ruviform resolved SHA・Cold/Warm)
- 対象 PR: https://github.com/scarecrow173/synvelis/pull/155
- 参照: ruviform `docs/impl/exp/issue-329-sccache-compile-cache.md`

## 1. 測定条件

| 項目 | 値 |
| --- | --- |
| 測定日 | 2026-11-04(UTC) |
| cold run | https://github.com/scarecrow173/synvelis/actions/runs/41823038077 (PR 初回 push、namespace `synvelis-v1-<RUNNER_OS>` 未存在) |
| warm run 1 | https://github.com/scarecrow173/synvelis/actions/runs/41824169427 (docs push、同一 Cargo.lock/SHA) |
| warm run 2 | https://github.com/scarecrow173/synvelis/actions/runs/41825678376 (exp/IR 追記 push) |
| kill switch 検証 | warm run 1 の `gw10-verify (ubuntu-latest)` を `CI_SCCACHE_DISABLE=1` 下で rerun(変数は検証後に削除済み) |
| sccache | v0.19.0(`mozilla-actions/sccache-action` pin `fc920bf0ec8de6ee65d409111f7ec508035751ba` = v0.0.11) |
| backend | GitHub Actions cache(`SCCACHE_GHA_ENABLED=true`・`SCCACHE_GHA_VERSION=synvelis-v1-<RUNNER_OS>`) |
| profile | dev・`CARGO_INCREMENTAL=0` |
| toolchain | rust 1.99.0(mise pin)。msrv-verify(1.89.0)は sccache 非導入で共有しない |
| Ruviform resolved SHA | `8fb665f168f8c1af011af849821a5929e5362629`(Cargo.lock pin・dep-guard single-SHA guard 通過) |
| Runner | GitHub-hosted ubuntu/windows/macos-latest |

## 2. Before 計測(main 直近 run、job_exec_s 秒)

| lane | run 41810580452 | 41809620685 | 41790537327 | 41816354694 |
| --- | --- | --- | --- | --- |
| verify ubuntu | 139 | 95 | 112 | 144(compile 中失敗: flake) |
| verify windows | 239 | 314 | 228 | 198 |
| verify macos | 194 | 194 | 192 | 143 |
| gw10 ubuntu | 47 | 46 | 43 | 35 |
| gw10 windows | 101 | 76 | 106 | 79 |
| gw10 macos | 56 | 53 | 36 | 63 |
| rt-verify | 92 | 95 | 85 | 91 |
| msrv-verify | 104 | 144 | 134 | 200 |
| ui-verify u/w/m | — | — | — | 36/88/54 |
| dep-guard | 22 | 23 | 23 | 25 |

## 3. Cold run 計測結果(run 41823038077)

### job wall time

| job | wall time | Before 対比 |
| --- | --- | --- |
| verify ubuntu | 144s | 変わらず |
| verify windows | 344s | +46〜146s(cold write + sccache setup 増分) |
| verify macos | 228s | +34〜85s(同上) |
| gw10 ubuntu | 51s | +4〜16s |
| gw10 windows | 89s | -17〜+13s |
| gw10 macos | 70s | +7〜34s |
| rt-verify | 79s | sccache 非導入 |
| msrv-verify | 134s | sccache 非導入 |
| ui-verify u/w/m | 30/100/56s | sccache 非導入 |
| dep-guard | 20s | 非対象 |
| evidence | 4s | 非対象 |

### sccache --show-stats(cold)

| job | requests | executed | hits | misses | hit rate | write errors | read/cache errors |
| --- | --- | --- | --- | --- | --- | --- | --- |
| verify ubuntu | 483 | 223 | 20 | 201 | 9.05% | 22 | 0 |
| verify windows | 481 | 225 | 3 | 220 | 1.35% | 47 | 0 |
| verify macos | 485 | 232 | 19 | 211 | 8.26% | 45 | 0 |
| gw10 ubuntu | 61 | 35 | 1 | 33 | 2.94% | 0 | 0 |
| gw10 windows | 58 | 34 | 0 | 33 | 0.00% | 26 | 0 |
| gw10 macos | 61 | 35 | 0 | 34 | 0.00% | 2 | 0 |

- cold hit 1-9%: 同一 job 内の `check`/`lint`/`test` phase 間で一致する
  compile 単位の再 hit + 先行 job の書込みが間に合った分。
- write errors 0-47(最大 windows verify): GHAC rate limit の既知挙動。
  fail-open により全 lane green を維持(実証済み)。
- verify windows の wall 増(+46〜146s)は cold write 競合と sccache
  setup/stats step の増分。warm run で回収できるかを次節で判定。

## 4. Warm run 計測結果(run 41824169427)

### job wall time(cold → warm、Before 対比)

| job | cold | warm | Before 4 run 範囲 | 判定 |
| --- | --- | --- | --- | --- |
| verify ubuntu | 144s | 112s | 95–144s(中央 ~126s) | 中央値より改善 |
| verify windows | 344s | 223s | 198–314s(中央 ~234s) | 中央値より改善(-11s) |
| verify macos | 228s | 177s | 143–194s(中央 ~193s) | 中央値より改善(-8%) |
| gw10 ubuntu | 51s | 35s | 35–47s | 範囲最良(-32% vs cold) |
| gw10 windows | 89s | 101s | 76–106s(中央 ~90s) | 範囲内・中央より微増 |
| gw10 macos | 70s | 69s | 36–63s(中央 ~55s) | Before 最大を微超過(+6s) |
| rt-verify | 79s | 85s | 85–95s | sccache 非導入・範囲内 |
| msrv-verify | 134s | 115s | 104–200s | sccache 非導入・範囲内 |
| ui-verify u/w/m | 30/100/56s | 35/76/45s | 36/88/54s(1 run) | sccache 非導入・範囲内 |
| dep-guard | 20s | 25s | 22–25s | 非対象 |
| evidence | 4s | 2s | — | 非対象・全緑 |

### sccache --show-stats(warm)

| job | requests | executed | hits | misses | hit rate | write errors | read/cache errors |
| --- | --- | --- | --- | --- | --- | --- | --- |
| verify ubuntu | 483 | 223 | 199 | 22 | 90.05% | 0 | 0 |
| verify windows | 481 | 225 | 167 | 56 | 74.89% | 0 | 0 |
| verify macos | 485 | 232 | 185 | 45 | 80.43% | 0 | 0 |
| gw10 ubuntu | 61 | 35 | 34 | 0 | 100.00% | 0 | 0 |
| gw10 windows | 58 | 34 | 29 | 4 | 87.88% | 0 | 0 |
| gw10 macos | 61 | 35 | 32 | 2 | 94.12% | 0 | 0 |

- warm hit rate 75–100%。cold run で書き込まれた entry が有効に
  再利用されている(OS 別 namespace のみで cross-lane 共有も成立:
  gw10 ubuntu は verify の書込みも含め 100%)。
- write/read/cache errors は全 lane で 0。cold の write error(0–47)は
  warm では再現せず — rate limit 競合は cold 同時書込み時の一過性。
- warm の verify wall time は Before 中央値を全 OS で下回る
  (ubuntu -14s・windows -11s・macos -16s)。cold→warm の改善は
  ubuntu -32s・windows -121s・macos -51s。
- gw10 は lane 合計でほぼ中立(ubuntu -16s・windows +12s・macos -1s)。
  compile 単位が小さい(35 executed)ため sccache setup/stats step の
  固定 overhead(~5–10s)が hit 節約と相殺される構造。critical path
  でないため総 wall には影響しない。

## 4a. Warm run 2 計測結果(run 41825678376、本 doc 追記 push)

| job | wall | executed | hits | misses | hit rate | errors |
| --- | --- | --- | --- | --- | --- | --- |
| verify ubuntu | 95s | 223 | 221 | 0 | 100.00% | 0 |
| verify windows | 222s | 225 | 213 | 10 | 95.52% | 0 |
| verify macos | 112s | 232 | 230 | 0 | 100.00% | 0 |
| gw10 ubuntu | 32s | 35 | 34 | 0 | 100.00% | 0 |
| gw10 windows | 97s | 34 | 32 | 1 | 96.97% | 0 |
| gw10 macos | 54s | 35 | 34 | 0 | 100.00% | 0 |

- warm run 1 で書き込まれた残りの miss 分が反映され hit 95.5–100% に
  収束(verify windows の残 miss 10 は cold write error 分 + 変動分)。
- wall も warm 1 より改善: verify u/w/m = 95/222/112s(ubuntu は
  Before 最小 95s と同値、macos は Before 最小 143s を下回る、
  windows は Before 範囲内で中央値 ~234s より -12s)、gw10 u/w/m =
  32/97/54s(ubuntu は Before 最小 35s を下回り、macos 54s は
  Before 範囲 36–63 内に収束 — warm 1 の 69s 超過は一過性だった
  ことが確認された)。
- 非対象 lane: rt 78s・msrv 132s・ui 35/104/39s・dep-guard 21s・
  evidence 3s — 全 PASS。

## 4b. Kill switch 実測

`CI_SCCACHE_DISABLE=1`(repository variable)を設定して run 41824169427 の
`gw10-verify (ubuntu-latest)` を単体 rerun:

- `Set up sccache` / `Export sccache GHA backend env` /
  `Verify sccache environment` / `sccache statistics` の 4 step が全て
  `skipped`。
- `Run GW-10 evidence-gated harness (mise run gw10)` を含む全 lane が
  `success` — RUSTC_WRAPPER 未設定の通常 compile 経路へ問題なく
  fallback。
- 検証後に変数は削除済み(恒久的な disable 状態を残さない)。

## 5. RT 列挙コンパイル分離計測

rt-verify(run 41823038077、sccache 非導入)の step 時間
(`gh api .../jobs` の steps より):

| step | 時間 | 内容 |
| --- | --- | --- |
| Require non-empty rt_ contract test selection | 56s | `cargo test --workspace --locked rt_ -- --list` — 全 test binary のコンパイル + 列挙実行 + strict-prefix カウント |
| Run RT contract tests (mise run test-rt) | 4s | `cargo test --workspace --locked rt_` — 同一 target dir のため再コンパイルなし(fingerprint 再利用)、`rt_` フィルタ実行のみ |

- 列挙コンパイル(56s)が rt-verify job の支配コストで、本試験(4s)は
  binary 再利用 + filter 実行のみ。job 内で重複コンパイルは発生して
  いない — sccache を導入した場合の効果は run 間の再利用に限られる。
- strict-prefix non-empty 選択は guard step 内で維持されている
  (変更なし)。sccache 非導入のため本変更による回帰なし。
- stage 2 で rt-verify に sccache を導入する場合、列挙 step は
  `synvelis-v1-Ubuntu` namespace で verify の `cargo test` compile
  単位と大部分を共有できる見込み(dev profile・同一 feature 集合・
  同一 workspace)。

## 6. 不変条件の確認

- 全 13 job green(cold run 実績)。10 required contexts の job 名・
  `evidence` needs・not-run-is-not-pass 判定は diff なし。
- `rt-verify`/`msrv-verify`/`ui-verify`/`dep-guard`/`evidence` に
  sccache step・`RUSTC_WRAPPER` 参照なし(diff 確認済み)。
- sccache は rustc wrapper であり hit しても check/clippy/test は全て
  実行される(検証 step の step 結果は全て `success`)。
- MSRV lane(rust@1.89.0)は sccache 非導入 — toolchain 成果物の
  誤共有は構造的に発生しない(compiler hash で entry 分離)。

## 7. 結論(採否判定)

- `verify`(3OS): **採用**。warm hit 75–100%(2 run)・write/read
  error 0・Before 中央値を全 OS で下回る wall。critical path の
  verify windows が Before 中央 -11〜-12s(2 warm run)。
  cold run も fail-open で全緑のため、cache backend 障害時の悪化
  リスクは setup overhead に限定される。
- `gw10-verify`(3OS): **採用**。hit 88–100%(2 run)・error 0。
  warm run 1 では lane 合計ほぼ中立、warm run 2 では u/w/m =
  32/97/54s で全 OS が Before 範囲内〜最良。compile 単位が小さい
  ため固定 overhead が支配的だが、verify との namespace 共有で
  hit は高く、warm 2 で macos の wall も範囲内に収束した。
  OS 別条件分岐の複雑さを正当化する差ではないと判断。
- `rt-verify` / `msrv-verify`: **今回不導入(分離設計のみ記録)**。
  MSRV 1.89.0 との toolchain 成果物誤共有を構造的に排除し、
  厳格 non-empty RT 選択(列挙 56s/本試験 4s)を変更なしで維持。
  stage 2 候補として exp §5 に分離方針を記録済み。
- `ui-verify`(3OS): **今回不導入**。npm install/test/build 主体で
  Rust compile 対象が限定的(ruviform でも ui lane hit 率低・分散大)。
  stage 2 以降で再評価。
- kill switch: `CI_SCCACHE_DISABLE=1` で sccache 全 step skip・通常
  compile へ fallback することを実測確認済み(§4b)。
- rollback: workflow から sccache step を除去する revert、または
  repository variable `CI_SCCACHE_DISABLE=1` で即時無効化可能。
  namespace `synvelis-v1-*` を上げれば cache 全破棄も可能。
