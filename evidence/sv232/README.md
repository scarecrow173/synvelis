# evidence/sv232

synvelis#232 棚卸し slice（AC1 台帳 + AC2 機能対応表 + AC5 分類規則初版）の
網羅性証拠。台帳本体は `docs/discovery/sv232-legacy-webview-inventory.md`。

| ファイル | 内容 |
| --- | --- |
| `enumeration.md` | 列挙コマンド・exit code・ヒット件数・対象 SHA・台帳項目との対応表 |
| `git-grep-webview.txt` | `git grep -n -i webview` の全ヒット（762 行） |
| `git-grep-webview-files.txt` | `git grep -l -i webview` のファイル一覧（188 ファイル） |
| `grep-ruviform-ui-webview.txt` | crate 名 `ruviform_ui_webview`/`ruviform-ui-webview` の直接ヒット |
| `grep-rvf-scheme.txt` | `rvf://` scheme ヒット |
| `grep-ci-ui.txt` | `.github/workflows/` の webview/ui 関連行 |
| `grep-mise-ui.txt` | `mise.toml` の webview/ui lane 定義 |
| `grep-tools.txt` | `tools/` の webview/wry/tao ヒット |
| `ls-files-ui.txt` | `git ls-files ui/`（追跡対象 34 ファイル） |
| `ls-files-mockups-assets.txt` | `git ls-files 'docs/mockups/**' 'assets/**'`（12 ファイル） |
