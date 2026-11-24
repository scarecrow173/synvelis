#!/usr/bin/env bash
# synvelis#633 — WebDAV 実 provider 検証 lane（docker/ 配下の補助 lane。
# mise.toml は触れないため mise task ではなくこのスクリプトが入口）。
#
# 実行内容:
#   1. docker compose config -q           — 構文検証
#   2. up -d --build                      — certgen + httpd 起動
#   3. curl contract checks               — TLS+Basic 認証越しに
#      PROPFIND(getetag)/GET(strong ETag)/If-Match 412/Range 206/401
#   4. webdav-probe                       — 実 WebDavProvider で
#      browse/stat/pin/open_pinned/open_preview を実走
#   5. down                               — コンテナ停止・削除
#
# 使い方（repo ルートからでも docker/ からでも可）:
#   bash docker/verify.sh
#
# verdict:
#   PASS     — すべての check が成功
#   FAIL     — 実際の検証失敗（exit 1）
#   NotRun   — docker 環境不在（exit 0。fake pass しない）
#   SETUP    — docker/.env 未作成などセットアップ不足（exit 2）
set -u
export MSYS_NO_PATHCONV=1

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"

COMPOSE="docker compose -f docker-compose.webdav.yml --env-file docker/.env"
BASE="https://localhost"
FAILED=0

check() { # name expected actual
  if [ "$2" = "$3" ]; then
    echo "[verify] PASS $1 ($3)"
  else
    echo "[verify] FAIL $1 (expected=$2 actual=$3)"
    FAILED=1
  fi
}

if ! command -v docker >/dev/null 2>&1 || ! docker version >/dev/null 2>&1; then
  echo "[verify] verdict=NotRun — docker が利用できない環境"
  exit 0
fi
if ! docker compose version >/dev/null 2>&1; then
  echo "[verify] verdict=NotRun — docker compose が利用できない環境"
  exit 0
fi
if [ ! -f docker/.env ]; then
  echo "[verify] verdict=SETUP — docker/.env がありません。"
  echo "         cp docker/.env.example docker/.env して値を設定してください。"
  exit 2
fi

set -a; . docker/.env; set +a
PORT="${WEBDAV_PORT:-8443}"
export WEBDAV_URL="${WEBDAV_URL:-$BASE:$PORT}"

echo "=== 1. docker compose config -q ==="
$COMPOSE config -q && echo "[verify] PASS compose config -q (exit 0)" || { echo "[verify] FAIL compose config -q"; exit 1; }

echo "=== 2. up -d --build ==="
$COMPOSE up -d --build || { echo "[verify] FAIL compose up"; exit 1; }

# readiness: TLS+認証を通して PROPFIND が 207 を返すまで待つ
CACERT="docker/webdav/certs/ca.crt"
AUTH="-u ${WEBDAV_USER}:${WEBDAV_PASSWORD}"
ready=0
for _ in $(seq 1 60); do
  # 判定は %{http_code} のみ（接続失敗時 curl 自身が 000 を印字）。
  # Windows schannel の curl が応答受領後に非 0 exit を返すことがある
  # ため exit status には依存しない。
  code=$(curl -s --ssl-no-revoke -o /dev/null -w "%{http_code}" --cacert "$CACERT" $AUTH -X PROPFIND -H "Depth: 0" "$WEBDAV_URL/" 2>/dev/null)
  [ "$code" = "207" ] && { ready=1; break; }
  sleep 1
done
if [ "$ready" != 1 ]; then
  echo "[verify] FAIL readiness — PROPFIND が 207 を返さない"
  $COMPOSE logs webdav | tail -30
  $COMPOSE down
  exit 1
fi

echo "=== 3. curl contract checks（補助証拠） ==="
# /tmp は Windows 版 curl に通じないため repo 内の一時 dir を使う
TMPD="docker/.verify-tmp"
mkdir -p "$TMPD"
trap 'rm -rf "$TMPD"' EXIT

code=$(curl -s --ssl-no-revoke -o /dev/null -w "%{http_code}" --cacert "$CACERT" -X PROPFIND -H "Depth: 1" "$WEBDAV_URL/library")
check "unauthenticated PROPFIND is 401" "401" "$code"

# stock 挙動では mod_dir が slashless collection へ 301 を返す（#634 の修正で
# provider はこれを追従する）。補助 curl も -L で追従し、最終的な 207 を見る。
code=$(curl -sL --ssl-no-revoke --cacert "$CACERT" $AUTH -X PROPFIND -H "Depth: 1" "$WEBDAV_URL/library" -o "$TMPD/propfind.xml" -w "%{http_code}")
check "PROPFIND Depth:1" "207" "$code"
grep -q "getetag" "$TMPD/propfind.xml" && echo "[verify] PASS multistatus carries getetag" || { echo "[verify] FAIL multistatus has no getetag"; FAILED=1; }

ETAG=$(curl -s --ssl-no-revoke -D - -o /dev/null --cacert "$CACERT" $AUTH "$WEBDAV_URL/library/sample.txt" | tr -d '\r' | sed -n 's/^[Ee][Tt][Aa][Gg]: //p')
echo "[verify] GET ETag: ${ETAG:-<none>}"
case "$ETAG" in
  \"*) echo "[verify] PASS strong ETag on GET" ;;
  *)   echo "[verify] FAIL strong ETag on GET"; FAILED=1 ;;
esac

code=$(curl -s --ssl-no-revoke -o /dev/null -w "%{http_code}" --cacert "$CACERT" $AUTH -H 'If-Match: "sv633-bogus-validator"' "$WEBDAV_URL/library/sample.txt")
check "If-Match bogus validator" "412" "$code"
code=$(curl -s --ssl-no-revoke -o /dev/null -w "%{http_code}" --cacert "$CACERT" $AUTH -H "If-Match: $ETAG" "$WEBDAV_URL/library/sample.txt")
check "If-Match pinned validator" "200" "$code"
code=$(curl -s --ssl-no-revoke -o /dev/null -w "%{http_code}" --cacert "$CACERT" $AUTH -H "Range: bytes=0-4" "$WEBDAV_URL/library/sample.txt")
check "Range bytes=0-4" "206" "$code"

echo "=== 4. webdav-probe（実 WebDavProvider） ==="
if command -v cargo >/dev/null 2>&1; then
  cargo run --quiet --manifest-path docker/webdav-probe/Cargo.toml || FAILED=1
else
  echo "[verify] FAIL cargo が見つからない — probe 未実施"
  FAILED=1
fi

echo "=== 5. down ==="
$COMPOSE down

if [ "$FAILED" = 0 ]; then
  echo "[verify] verdict=PASS"
else
  echo "[verify] verdict=FAIL"
fi
exit "$FAILED"
