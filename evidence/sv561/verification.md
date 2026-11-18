# sv561 verification evidence

Issue: synvelis#561 — op_facts under-report（観測不能 cascade
削除の unsupported 欠落）修正の検証記録。

- OS: Windows (worktree `sv561-opfacts-underreport`)
- Toolchain: `mise` 管理（`mise.toml` / `mise.lock` の pin）
- 実施日: 2026-11-18
- Base: `origin/main` `41d85021` / Branch: `bugfix/561-opfacts-unverifiable-cascade`

## red→green（修正前後の実測）

`evidence/sv561/reproduction.md` に記録。

修正前 (`41d85021`) に pin test を追加して実行した結果、
`set-gain-target` clear/rebind・`track.remove`・`midi.source.
remove` の各 denied 系 test は `Completed`（under-report）で
失敗 = red。`prepare.rs` の op_facts 3 arm 修正後に
`PreparationRejected` "unverifiable" となり全 pin green。

review-protocol で追加した 2 件（Composite 逐次評価・
`monitoring.tracks` decl）は、修正前の pin 追加時点で
`Completed` 受理を実測して red を確認してから修正した
（`sv561_batch_seal_release_unobservable_denied`・
`sv561_track_remove_monitor_decl_denied` — いずれも
pinned-base 版 Composite 評価・decl 非検査のコードで
`Completed` を返すことを確認）。

## 実行した検証（全て exit 0 / Pass）

| コマンド | 結果 |
| --- | --- |
| `mise run check synvelis-gateway` | Pass |
| `mise run lint synvelis-gateway` | Pass（clippy 指摘 2 件を修正後 Pass） |
| `mise run test synvelis-gateway` | Pass（全 integration binary、sv561 pin 10 件 + cb06 sv#550/sv#553/sv#557 非回帰含む） |
| `mise run check synvelis-project` | Pass |
| `mise run lint synvelis-project` | Pass |
| `mise run test synvelis-project` | Pass（sv561_unobservable_cascade 7 件含む） |
| `mise run fmt-check` | Pass |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI framework dep なし） |
| `mise exec -- prek run --all-files` | Pass（全 hook） |

## NotRun / Unsupported

- GUI-L2（desktop build/smoke）: POLICIES.md §5 により日常
  必須検証外。本変更は headless の prepare 層のみで GUI
  非接触。Daily/手動 Full CI に委譲。
- EXT-L3（外部依存 E2E）: 対象外（外部依存を持たない
  prepare 層の変更）。
- MSRV・3OS 横断: POLICIES.md §5 により日常必須検証外。
  Daily/手動 Full CI に委譲。
- `docs/**/README.md` index 再生成: 本 slice の lease 外
  （doc chain は type 別ディレクトリに配置済み）。
