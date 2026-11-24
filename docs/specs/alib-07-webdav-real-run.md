---
id: "SPEC-alib07-webdav-real-run"
type: "spec"
status: "implemented"
title: "ALIB-07 — WebDAV 実 provider（docker/compose）による fault matrix 実走手順（synvelis#633）"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/633"
    - "https://github.com/scarecrow173/synvelis/issues/91"
  changes:
    added:
      - type: "file"
        path: "docs/specs/alib-07-webdav-real-run.md"
      - type: "file"
        path: "docker/verify.sh"
      - type: "file"
        path: "docker/webdav-probe/src/main.rs"
---

# WebDAV 実 provider による fault matrix 実走手順

## 対象と位置づけ

`docs/test-specs/alib-07-fault-matrix.md` の WebDAV 経路で「実 WebDAV server は NotRun」と注記された行を、docker で立ち上げた実 Apache `mod_dav` サーバへ実 `WebDavProvider` を通して実走するための手順を定める。本書は環境と手順の提供であり、matrix 本体（`docs/test-specs/`）の verdict 更新は後続 gate slice が行う。実走で使う構成の選定根拠は `docs/designs/alib-07-webdav-docker-provider.md`、実測記録は `evidence/sv633/` にある。

## 対象 Case

| matrix 行 | 実サーバで検証できる部分 |
| --- | --- |
| LIB-PIN-03（WebDAV If-Match strong/weak） | strong ETag の pin・`If-Match` conditional GET・412→`PreconditionFailed`・weak/missing ETag の fail closed（weak は fixture 調整が要るため後述の制約を参照） |
| LIB-PIN-05（auth revoke / permission denied）のうちサーバ起因の部分 | 401→`AuthExpired`（誤 credential）・403 系（権限設定を変えた場合） |
| browse/stat/preview 系の行（scripted mock を実サーバへ置き換える確認） | `coverage=Complete` の browse・`byte_len` の stat・Range preview |

## 前提

- Docker Desktop（docker compose v2 以降）が動作すること。
- 初回のみ `cp docker/.env.example docker/.env` して `WEBDAV_USER` / `WEBDAV_PASSWORD` を設定する。ポートを変える場合は `WEBDAV_PORT` も設定する。
- リポジトリルートをカレントにして実行する（probe は `WEBDAV_CA` の相対パスをカレントから解決する）。
- cargo が PATH 上にあること（probe は workspace 外の独立 crate で、root `Cargo.toml`/`Cargo.lock` を変更しない）。

## 手順

### 一括 lane

```sh
bash docker/verify.sh
```

`config -q` → `up -d --build` → curl contract checks（補助証拠）→ probe → `down` までを一括で行い、終了時に `verdict=PASS/FAIL/NotRun/SETUP` を印字する。docker が使えない環境では `NotRun` で exit 0、`.env` 未作成では `SETUP` で exit 2 を返し、失敗を Pass に偽装しない。

### 個別操作

```sh
# 起動（certgen が初回のみ証明書・htpasswd を docker/webdav/certs/ に生成）
docker compose -f docker-compose.webdav.yml --env-file docker/.env up -d --build

# 実 provider での接続確認
set -a && . docker/.env && set +a
cargo run --manifest-path docker/webdav-probe/Cargo.toml

# 停止
docker compose -f docker-compose.webdav.yml --env-file docker/.env down
```

probe は browse（Root scope）→ stat → pin → open_pinned → open_preview の順に実走し、続けて誤 credential（`AuthExpired` 期待）と foreign CA（`TlsVerification` 期待）の negative check を行う。最後に `PROBE PASS` が出れば成功である。

### verdict の記録方法

実走者は実行コマンド・exit code・観測を `evidence/sv633/`（または実走時点の evidence ディレクトリ）に記録し、matrix の対象行の verdict を「Pass: scripted mock」から「実 WebDAV server（`docker-compose.webdav.yml`、Apache `mod_dav`）で実走」と更新する。未実施の軸は推測せず `NotRun` と理由を残す。`evidence/sv633/verification.md` が記録例である。

## 接続設定の正本

| 項目 | 値 / 置き場所 |
| --- | --- |
| endpoint | `https://localhost:${WEBDAV_PORT}`（既定 8443） |
| `root_path` | `/library`（`WEBDAV_ROOT` で変更可。fixture は `docker/webdav/data/library/`） |
| trust 用 CA | `docker/webdav/certs/ca.crt`（certgen が生成。`TlsTrust::from_pem` の入力） |
| 認証 | `docker/.env` の `WEBDAV_USER`/`WEBDAV_PASSWORD` → `Authorization: Basic <base64(user:pass)>` を `OpaqueCredential` として `CredentialResolver` が返す |
| TLS negative 材料 | `docker/webdav/certs/ca-foreign.crt`（無関係な CA。`WEBDAV_FOREIGN_CA`） |

## 制約と既知事項

- `docker/webdav/conf/webdav.conf` は `DirectorySlash Off` を指定している。provider が redirect 解決で trailing slash を落として 301 ping-pong になる不具合（`crates/` 側・`scarecrow173/synvelis#634` で起票）への環境側 workaround であり、`On` に戻すと再現する。provider 修正後は `On` での再検証が望ましい。
- fixture は read-only のため、PUT/DELETE/LOCK・weak ETag の注入（Apache の `FileETag` 調整で可能だが本環境では未配線）・権限別 403 は環境の現構成では扱わない。これらは実走記録では `NotRun` とする。
- Windows 版 curl（schannel）は私有 CA の revocation check で失敗するため、スクリプト内の curl は全て `--ssl-no-revoke` 付きである（他プラットフォームでは無害）。
- 生成物（`docker/webdav/certs/`・`docker/.env`・probe の `target/`）は gitignore 済みであり、実値・秘密鍵はコミットしない。
