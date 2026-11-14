# SF-05 ui/ slice verification results（Issue #108 / synvelis#108）

OS: Windows 11（worktree `sv108-ui-viewmodel`、branch
`scarecrow173/sv108-ui-viewmodel`）。toolchain は `mise.toml`
管理（Node 26.12.1、TypeScript ~5.9.3、Vitest ~5.0.5、
happy-dom ~21.0.4）。canonical = `mise run` 経由。

変更は `ui/src/ipc.ts`（OpName union）・`ui/src/viewmodel/`
新規 2 ファイル・`ui/test/` 新規 2 spec + conformance 追記・
docs/evidence のみ。`crates/`・`ui/src/store.ts`・`app.tsx`・
`views/`・`components/`・`package.json`/`tsconfig`/`vite.config`・
`Cargo.*`・`mise.toml` は未変更のため、検証は ui/ lane に比例
（POLICIES.md §5）。cargo レーンは crates 未変更のため本 slice
の必須検証に含めない（実行せず）。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run sync-ui-deps` | Windows | node tools/sync-ruviform-ui.mjs | 0 | Passed（vendor stale 検出のため実施。`ui/vendor/ruviform` を pin `fd1d4d5f4d` へ再同期。生成物・untracked） |
| `mise run check-ui` | Windows | tsc --noEmit + sync check | 0 | Passed（`tsc --noEmit -p tsconfig.json` エラー 0） |
| `mise run test-ui` | Windows | vitest run | 0 | Passed（8 test files / 73 tests 全 pass。新規 2 spec = 17 tests、既存 56 tests green 維持） |
| `mise run fmt-check` | Windows | cargo fmt --all -- --check | 0 | Passed |
| `mise exec -- prek run --all-files` | Windows | prek hooks | 0 | Passed（trim/eof/yaml/toml/mixed-eol/BOM/private-key/actionlint 全 Passed） |

## 補助証拠

- 反復中に `npx vitest run`（ui/node_modules 未 install 時に
  vitest@5.1.0 を npx が取得して config 解決失敗）を 1 回
  実行したが、canonical は `mise run test-ui`（`install-ui`
  depends で `npm ci` 済みのローカル依存を使用）。最終結果は
  上記 mise タスクの実測値。
- `sync-ui-deps` 初回 `test-ui` で vendor REV 不一致
  （vendored=20a24a66… pin=fd1d4d5f…）を fail-closed 検出し、
  `sync-ui-deps` 再実行で復旧（`check-ruviform-rev` の想定
  経路、synvelis#230）。

## Commit SHA

- （push 後に追記 — branch `scarecrow173/sv108-ui-viewmodel`）

## 未検証（NotRun/Unsupported）

- `mise run build-ui`: views/app/components 未変更のため
  **NotRun/不要**（slice 指定どおり）。
- cargo レーン（`check`/`lint`/`test`/`build`）: `crates/` 未変更
  のため必須検証に含めず未実行。実行する場合も補助証拠扱い。
- `*-msrv`・`*-full`・Feature Matrix・3OS 横断・Golden/長時間
  回帰: 日常検証の対象外（Daily/Manual Full CI の責務、
  ユーザー指定「MSRV レーンは通常開発では行わず」）。
- 実 WebView host e2e・実 GPUI window・実 2-window・
  SF-UX-08/Windows L2（`scarecrow173/ruviform#631` harness・
  `scarecrow173/synvelis#114` gate）: **NotRun**。headless
  Pass を製品受入へ昇格しない。
- `scarecrow173/synvelis#109`/`scarecrow173/synvelis#103`
  oracle join・`evidence/reports/sf-06-cross-gate-ledger.md`:
  **NotRun**（#109 管轄、touch しない）。
- 統合検証: merge 後 main SHA の必須 CI 結果が証拠
  （Coordinator 確認）。ローカル検証は変更範囲の検証。
