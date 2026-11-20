# SV#602 ruviform pin 84a68f25 → cd4213a1 差分精査メモ

実施日: 2026-11-20 / Worker worktree: `G:/orca/workspaces/synvelis/sv602-rev-bump`
branch: `chore/602-ruviform-rev-bump`（base `origin/main` = `ea763ab4`）
rev-watch Owner tracker: `scarecrow173/synvelis#28`（常設・close しない）

## candidate SHA 再解決

- `git -C G:/repositry/audio-products/ruviform fetch origin` →
  `git rev-parse origin/main` =
  `cd4213a1d665600f6adfefa0c3a90f2be9b4e4ce`（sha8 `cd4213a1`）。
  Spec 起稿時点 candidate と一致。upstream 前進なし。
- 目的 commit の含有確認（log 走査）: rf#823 修正
  `b202f9ab`（merge `32477e3f`）・rf#824 `7853ccd2`・
  rf#631 `a56a0be9` はいずれも `84a68f25..cd4213a1` 内に存在。
  現 pin `84a68f25` は rf#823 修正を含まない ancestor（探索側で
  `git merge-base` 確認済み）。
- `git rev-list --count 84a68f25..cd4213a1` = **73 commits**。

## upstream diff 規模

- `git diff --stat 84a68f25..cd4213a1 -- crates/`:
  **107 files changed, +19,040 / −1,193**。大半は
  dep-tree-external の `ruviform-plugin-test-{clap,vst3}`・
  `ruviform-plugin-dock-test`・`ruviform-plugin-vst3`・
  `ruviform-plugin-clap`・`ruviform-plugin-host` tests と
  `ruviform-ui-test` の新規 bin `rvf_ctl`。
- docs/evidence/索引系・apm・`mise.toml` は consumer API 非影響。

## synvelis 依存木との交差

lockfile の ruviform package は **30 のまま不変**
（`grep -c 'name = "ruviform-' Cargo.lock` = 30）。今回の範囲で
新規 crate の dep-tree 入りはなし。

dep-tree 内 crate ∩ upstream `src/` 差分あり = **8 crate**:

| crate | upstream 差分 | synvelis 消費面 | 破壊的変更の有無と影響 |
| --- | --- | --- | --- |
| `ruviform-media-reader` | `prefetch.rs` +18/−1: `Prefetch::manual` の free-ring seed を `debug_assert!(…try_push…is_ok())` から `free_tx.try_push(block).expect("free ring capacity equals pool size")` へ分離（**rf#823 — 本 bump の主目的**） | `synvelis-app`/`synvelis-player`/`synvelis-render`（frozen bench Arm B の HeadlessDriver 経路） | なし（内部実装修正）。release/bench profile でも free pool が初期化されるようになり、`[profile.bench.package.ruviform-media-reader] debug-assertions = true` workaround が撤去可能に |
| `ruviform-audio` | `typed/probe.rs` +7/−1: `emit_capture` の `chunks_out.try_push` を `debug_assert!` から分離・`expect` 化（rf#824） | app/player/project/recording/render/workspace | なし（内部実装修正・API 不変） |
| `ruviform-audio-test` | `metadata.rs` +59/−9: `EnvironmentKind::RealAbiTestPlugin` の serialize/`as_str` を `"real-abi-test-plugin"` → `"test-plugin"`（Case Schema v1 語彙、#725 Phase A/B。deserialize は `alias` で旧 spelling も受理）・`RunMetadata::for_environment` 追加（#629 AC3）・`RunMetadata::deterministic` doc 更新 | `synvelis-recording` dev-dep（`backend`/`clock`/`oracle`/`pcm` のみ消費 — `metadata` module 未使用を grep 確認） | **公開 API 破壊なし**。serde serialize 語彙の変更あり（`test-plugin` へ統一）だが synvelis は当該 type を消費せず影響なし。deserialize は旧 spelling 互換 |
| `ruviform-exec` | `executor.rs` 新規 +1174（#744 bounded 非 RT `JobExecutor` — `ExecutorConfig`/`ExecutorError`/`JobExecutor`/`QueuedJob`/`SubmitError` を `pub use`）、`job.rs` +86/−30（private item の内部再配置）、`lib.rs` doc+re-export | player/render/workspace | なし。純粋 additive（新規 `pub mod executor`）。**新 API 採用判断: 不採用**（YAGNI — `JobExecutor` 導入は別 slice 判断事項） |
| `ruviform-math` | `dsp.rs` +2/−2（doc 内の provenance パス名更新のみ）+ `Cargo.toml` に `[[bench]] dsp_frozen` 追加（rf#807 sibling lane — consumer 非影響） | 直接 dep なし（推移的） | なし（doc のみ） |
| `ruviform-plugin-core` | `boundary.rs` +109（`GuiKeyDirection`/`GuiKeyEvent`/`GuiModifiers`/`GuiSize` 新規 pub 型 — #631/#389 editor verb 語彙）、`plugin.rs` +93/−3、`lib.rs` re-export 追加 | app/player/project/workspace | なし。純粋 additive（新規型・新規 re-export のみ） |
| `ruviform-plugin-host` | `command.rs` +56（`HostCommand` に `SetGuiSize`/`SetGuiVisibility`/`SetGuiParent`/`SetGuiFocus`/`SendGuiKey` variant 追加 — enum は `#[non_exhaustive]`）、`editor_session.rs` +187（`EditorSessionOps` に `set_focus`/`send_key` 追加 — #631）、`composite.rs`/`host.rs`/`lib.rs` 配線 | app/player（`EditorSessionOps`・editor verb 系は synvelis 未実装・未消費を grep 確認） | なし。`#[non_exhaustive]` enum の additive variant + trait method 追加（synvelis は当該 trait を実装しないため required-method 追加の影響なし）。**新 API 採用判断: 不採用**（editor verb 経路の採用は別 slice） |
| `ruviform-ui-test` | `src/bin/rvf_ctl.rs` 新規 +1079（#805 制御 CLI）+ `Cargo.toml` `[[bin]] rvf-ctl`・dev-dep `proptest` 追加 | app | なし（新規 bin のみ・lib 不変） |

## pub 削除スキャン結果

`git diff 84a68f25..cd4213a1 -- 'crates/*/src/**'` の削除行
（**770 行**）に対し `pub` を含む行は **5 件のみ**で全て
internal visibility: `pub(super) fn attach`×2（plugin-test-vst3
内・dep-tree-external）、`pub(crate)` 3 件（同上）。
**consumer 向け `pub` item の削除・rename・signature 変更は
本範囲に存在しない** — dep-tree 内は全て additive/内部修正。

## 依存グラフ変化（consumer に伝播するもの）

- **`ruviform-ui-gpui` の `gpui` dep: crates.io `0.2.2` →
  `git+https://github.com/scarecrow173/gpui?tag=v0.2.2-rf1`**
  （rf#532 Option-A — crates.io 0.2.2 + DirectComposition target
  非 topmost 化 1 行 patch の fork tag。`version = "=0.2.2"` で
  base を assert）。git dep のため Cargo.lock 経由で consumer に
  伝播 — synvelis manifest 変更不要。lock 上は registry source +
  checksum 行が git source `#f77e44e` に置換。
  `mise run check-app-native`（GPUI lane spot check）で検証済み。
- `ruviform-audio-test` dev-dep 追加
  （`ruviform-audio-device`/`ruviform-midi-device`）・
  `ruviform-ui`/`ruviform-ui-test` dev-dep `proptest` 追加 — 全て
  **dev-dependency のため consumer の dependency resolution に
  波及しない**（non-workspace package の dev-dep は lock に
  入らない。proptest は synvelis lock に sv#601 由来で既存）。
- `ruviform-plugin-test-clap` の `clack-extensions` feature 追加・
  `ruviform-plugin-{clap,vst3,dock-test}` manifest 変更は
  dep-tree-external（synvelis 非影響）。
- upstream `Cargo.toml` に `workspace.lints.rust.unexpected_cfgs`
  `cfg(kani)` 追加（#806）— ruviform 側 lint 設定で consumer 非影響
  （synvelis 側は sv#601 slice 1 で同等設定済み）。

## Cargo.lock 差分

scoped `cargo update -p`（30 package 全指定・`--precise` 不使用・
ruviform main HEAD 解決）後の `git diff Cargo.lock`:
**31 insertions / 32 deletions**。

- 30 package × `source = "git+…ruviform.git#84a68f25…"` →
  `#cd4213a1d665600f6adfefa0c3a90f2be9b4e4ce` の fragment 更新。
- `gpui` package: `source = "registry+…"` + `checksum` 行削除 →
  `source = "git+https://github.com/scarecrow173/gpui?tag=v0.2.2-rf1#f77e44e64b66c1bcc4757e68ee5dfc903fb96139"`。
- 上記以外の差分なし — crates.io version drift・dep 追加・
  package 増減・無関係 source 変更は **検出されず**。
- rollback: `cargo update` は lockfile のみの変更のため、
  `git checkout -- Cargo.lock`（＋ worktree 上の `Cargo.toml`
  override 復元）で `84a68f25` へ完全に戻せる。

## 検証 lane との対応

- `check-app-native`: gpui fork `f77e44e` を含む native feature 型
  チェック — exit 0（GPUI lane spot check として機能）。
- `bench-frozen`: workaround 撤去後の bench profile で
  `Prefetch::manual` が正しく初期化されることを実証（Arm K 全 cell
  `errors=0` — underrun 無音なし）。
- `test-rt`（workspace 広範 rt_ contract）・`test synvelis-player`
  （HeadlessDriver 本番経路）・`check-headless-deps` 全て pass。
