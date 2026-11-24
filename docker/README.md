# docker/ — WebDAV 実 provider 検証環境（synvelis#633）

ローカル/opt-in の検証 lane。CI には組み込まない。
`docs/test-specs/alib-07-fault-matrix.md` の「実 WebDAV server は
NotRun」注記行を実走するための、TLS・Basic 認証付き Apache
httpd + mod_dav サーバを docker compose で立てる。

- 仕様・手順・接続設定: `docs/specs/alib-07-webdav-real-run.md`
- イメージ選定根拠: `docs/designs/alib-07-webdav-docker-provider.md`
- 実測記録: `evidence/sv633/verification.md`

## 構成

| パス | 役割 |
| --- | --- |
| `../docker-compose.webdav.yml` | certgen + webdav の 2 service 定義（ルート） |
| `webdav/Dockerfile` | `httpd:2.4.71-alpine` へ ssl/dav module を有効化 |
| `webdav/conf/webdav.conf` | 443 vhost・`Dav On`・Basic 認証・証明書パス |
| `webdav/certgen.sh` | throwaway 私有 CA・サーバ証明書・htpasswd の初回生成 |
| `webdav/certs/` | 生成物の置き場（gitignored — `docker/.gitignore`） |
| `webdav/data/` | 検証用 fixture（`/library` 配下の読み取り専用データ） |
| `webdav-probe/` | 実 `WebDavProvider` を通す接続 probe crate |
| `.env.example` | `docker/.env` のテンプレ（実値はコミットしない） |
| `verify.sh` | up → contract check → probe → down の一括実行 |

## 使い方

```sh
cp docker/.env.example docker/.env   # 初回のみ。値を編集する
bash docker/verify.sh                # 一括実行（推奨）
```

個別実行（repo ルートから）:

```sh
docker compose -f docker-compose.webdav.yml --env-file docker/.env up -d --build
set -a && . docker/.env && set +a
cargo run --manifest-path docker/webdav-probe/Cargo.toml
docker compose -f docker-compose.webdav.yml --env-file docker/.env down
```

## 接続設定（provider 側）

| 項目 | 値 |
| --- | --- |
| `WebDavUrl` | `https://localhost:<WEBDAV_PORT>`（default 8443） |
| `WebDavConfig.root_path` | `/library` |
| `TlsTrust::from_pem` | `docker/webdav/certs/ca.crt`（生成された私有 CA） |
| `credential_key` | 任意の key 名。resolver が `Basic <b64(user:pass)>` を返す |

証明書・鍵・htpasswd は `docker/webdav/certs/` に生成されるだけで
コミットされない（`docker/.gitignore` + `detect-private-key`）。
作り直すときは `certs/` を空にして `up` し直す。
