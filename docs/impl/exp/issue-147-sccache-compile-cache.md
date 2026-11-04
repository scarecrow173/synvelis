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
| warm run | (採取中) |
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

## 4. Warm run 計測結果

(採取中 — 次回 push/rerun 後に追記)

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

(warm run 採取後に記入)
