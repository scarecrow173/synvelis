---
id: "DSN-alib07-webdav-docker-provider"
type: "design"
status: "approved"
title: "ALIB-07 — WebDAV 実 provider 検証環境のイメージ選定と構成設計（synvelis#633）"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/633"
    - "https://github.com/scarecrow173/synvelis/issues/91"
    - "https://github.com/scarecrow173/synvelis/issues/90"
  changes:
    added:
      - type: "file"
        path: "docker-compose.webdav.yml"
      - type: "file"
        path: "docker/webdav/Dockerfile"
      - type: "file"
        path: "docker/webdav/conf/webdav.conf"
      - type: "file"
        path: "docker/webdav/certgen.sh"
      - type: "file"
        path: "docker/webdav-probe/Cargo.toml"
      - type: "file"
        path: "docker/webdav-probe/src/main.rs"
      - type: "file"
        path: "docker/verify.sh"
      - type: "file"
        path: "docs/specs/alib-07-webdav-real-run.md"
      - type: "file"
        path: "evidence/sv633/verification.md"
---

# WebDAV 実 provider 検証環境のイメージ選定と構成設計

## 背景と目的

`synvelis#91` の fault matrix（`docs/test-specs/alib-07-fault-matrix.md`）は、WebDAV 経路の残 gate として「実 WebDAV server は NotRun」の注記行を残している（代表例は LIB-PIN-03）。`synvelis#90` で https/TLS transport は main 済みであり、不足しているのは制御可能な実サーバと、実 `WebDavProvider` をそこへ向かせる入口の二つである。本設計は `docker compose` で立ち上がる TLS・認証付き WebDAV サーバと、同環境を検証する補助 lane の構成を定める。

## 選定軸

fault matrix の実走対象契約を、イメージ選定の必須軸とする。

- PROPFIND `Depth:0/1` が `207 multistatus` を返し、各 response に `getetag` を含む（`pin` は PROPFIND 経由で ETag を採るため、GET の ETag だけでは足りない）。
- strong ETag を返す（weak `W/`・欠落は fail closed 対象）。
- `If-Match` を解釈し、不一致に `412`・一致に `200` を返す。
- `Range` GET に `206` + `Content-Range` を返す。
- TLS（自己署名 CA 構成でよい）と Basic 認証を設定できる。
- これらに加えて、イメージの保守状況・タグの新しさ・設定の見通し・サイズを比較軸とする。

## 候補と実測結果

| 候補 | 評価 | 結果 |
| --- | --- | --- |
| `sigoden/dufs:v0.47.0` | 実測。PROPFIND 207・GET strong ETag・Range 206・`If-Match` 412 は全て満たすが、PROPFIND の multistatus が `getetag` を返さない（`allprop`・prop 指定の双方で確認） | 不採用 — `pin` の前提を満たせない |
| `bytemark/webdav` | Apache `mod_dav` の env 駆動ラッパー。契約面は httpd と同等だが、配布元が非公式で更新頻度が低く、TLS 設定が `SSL_CERT=selfsigned` の opaque な生成に限られる | 不採用 — 公式 `httpd` で同等構成を書けるため |
| `wsgidav` | Python 製。ETag が内容由来でなく mtime/size ベースで再現性の制御が弱く、strong validator の保証を選定根拠として裏付けづらい | 不採用 |
| nginx + `ngx_http_dav_module` | PROPFIND の対応が拡張モジュール依存で、`getetag` の liveprop 応答が標準構成で保証できない | 不採用 |
| `httpd:2.4.71-alpine`（`mod_dav` + `mod_ssl`） | 実測で全軸を満たす（evidence 参照）。Apache `mod_dav` は参照実装に近く、公式イメージで保守が続いている | 採用 |

dufs と httpd の二者は実際にコンテナを立てて curl で計測した。dufs が落とされた理由は機能の差ではなく契約の差であり、`pin` が要求する「PROPFIND が `getetag` を返す」を満たさない一点による。

## 構成設計

### コンテナ構成

`docker-compose.webdav.yml` は `certgen` と `webdav` の二サービス構成である。`certgen`（`alpine/openssl:3.5.9`、一度だけ走る init）は私有 CA・サーバ証明書・`htpasswd`・foreign CA（TLS negative check 用の無関係な CA）を `docker/webdav/certs/` へ生成する。`webdav`（`synvelis-webdav-httpd:local` を build）は証明書を mount して `https://localhost:8443` で待ち、`/library` 以下に fixture データ（`docker/webdav/data/library/` を bind）を read-only で公開する。

### 秘密情報の配置

生成物はすべて `docker/webdav/certs/`（gitignored）に閉じる。Basic 認証の実値は `docker/.env`（gitignored）に置き、テンプレートを `docker/.env.example` として追跡する。`detect-private-key` が fail する配置（秘密鍵のコミット）は構造上起きない。証明書を初回 up の init で都度生成する方式を選んだのは、生成物をコミットしない方針と手順ゼロで立ち上がることの両立のためである。

### 接続 probe

実 provider を通す入口が `synvelis-cli` に存在しないため、`docker/webdav-probe/` に `[workspace]` 宣言で独立させた小さな crate を置いた。path 依存で `synvelis-library` を参照し、`WebDavUrl::parse` → `WebDavConfig{root_path:"/library"}` → `TlsTrust::from_pem(ca.crt)` → `CredentialResolver`（env 由来の `Authorization: Basic …`）で `ProviderContext` を組み立てる。workspace から独立させるのは、root `Cargo.toml`・`Cargo.lock` が編集範囲外であるためであり、probe 側の `Cargo.lock` は ruviform の解決 rev を root lock に合わせた上で追跡対象とする。

### `DirectorySlash Off` による回避（計測上の注意）

実 Apache への接続で provider 側の不具合を一つ発見した。provider は collection を canonical path `/library`（trailing slash なし）で PROPFIND するが、`mod_dir` は `/library/` への `301` を返し、provider の redirect 解決が `normalize_path` で slash を再び落とすため、同一 redirect が回数上限まで繰り返される。修正は `crates/`（本 slice の範囲外）に属するため、環境側では `webdav.conf` に `DirectorySlash Off` を指定して slashless PROPFIND へ `207` を直接返させた。provider 側の修正は `scarecrow173/synvelis#634` として起票した。この設定は検証をごまかすものではなく、PROPFIND・ETag・`If-Match`・Range の各契約はそのまま実測されている。stock 挙動（`On`）への復帰で不具合を再現できる。

## やらないこと

- fault matrix 本体の verdict 更新と matrix の実走（後続 gate slice の責務）。
- CI workflow への組込み（本環境はローカル/opt-in lane）。
- WebDAV への書き込み系メソッド（PUT/DELETE/LOCK）の契約検証 — fixture は read-only で提供し、provider の公開 API も読み取り系のみを対象とする。
