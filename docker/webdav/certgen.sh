#!/bin/sh
# alpine/openssl コンテナ内で一度だけ走る初期化スクリプト。
# docker/webdav/certs/（gitignored・/certs に bind）へ、
#   ca.key / ca.crt   : throwaway 私有 CA（probe の TlsTrust::from_pem 入力）
#   server.key / server.crt : CA が署名したサーバ証明書（SAN: localhost/127.0.0.1/::1）
#   htpasswd          : Basic 認証エントリ（openssl passwd -apr1）
# を生成する。生成済みなら何もしない（idempotent）。
# 作り直すときは docker/webdav/certs/ を空にしてから up し直す。
set -eu

cd /certs

if [ ! -f ca.crt ] || [ ! -f ca.key ]; then
  rm -f ca.crt ca.key server.crt server.key server.csr server.ext ca.srl
  openssl req -x509 -newkey rsa:2048 -nodes -sha256 -days 3650 \
    -subj "/CN=Synvelis WebDAV Test CA" \
    -addext "basicConstraints=critical,CA:true" \
    -addext "keyUsage=critical,keyCertSign,cRLSign" \
    -keyout ca.key -out ca.crt
fi

if [ ! -f server.crt ] || [ ! -f server.key ]; then
  rm -f server.crt server.key server.csr server.ext ca.srl
  openssl req -newkey rsa:2048 -nodes -sha256 \
    -subj "/CN=localhost" -keyout server.key -out server.csr
  printf '%s\n' \
    "basicConstraints=critical,CA:false" \
    "keyUsage=critical,digitalSignature,keyEncipherment" \
    "extendedKeyUsage=serverAuth" \
    "subjectAltName=DNS:localhost,IP:127.0.0.1,IP:::1" \
    > server.ext
  openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
    -days 825 -sha256 -extfile server.ext -out server.crt
  rm -f server.csr server.ext ca.srl
fi

# probe の TLS negative check 用に、サーバ証明書の署名 CA とは
# 無関係な別 CA も生成する（信頼されない anchor の材料）。
if [ ! -f ca-foreign.crt ] || [ ! -f ca-foreign.key ]; then
  rm -f ca-foreign.crt ca-foreign.key
  openssl req -x509 -newkey rsa:2048 -nodes -sha256 -days 3650 \
    -subj "/CN=Synvelis WebDAV Foreign CA" \
    -addext "basicConstraints=critical,CA:true" \
    -addext "keyUsage=critical,keyCertSign,cRLSign" \
    -keyout ca-foreign.key -out ca-foreign.crt
fi

if [ ! -f htpasswd ]; then
  : "${WEBDAV_USER:?WEBDAV_USER is required}"
  : "${WEBDAV_PASSWORD:?WEBDAV_PASSWORD is required}"
  printf '%s:%s\n' "$WEBDAV_USER" "$(openssl passwd -apr1 "$WEBDAV_PASSWORD")" > htpasswd
fi

echo "certgen: /certs ready (ca.crt, ca-foreign.crt, server.crt, server.key, htpasswd)"
