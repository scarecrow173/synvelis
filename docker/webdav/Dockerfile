# Apache httpd + mod_dav による TLS・Basic 認証付き WebDAV サーバ。
# 公式イメージのデフォルト httpd.conf では ssl/dav 系 module が
# コメントアウトされているため、sed で有効化して自前の vhost を
# conf/extra/webdav.conf として追加する。
FROM httpd:2.4.71-alpine

RUN sed -i \
      -e 's/^#LoadModule ssl_module /LoadModule ssl_module /' \
      -e 's/^#LoadModule socache_shmcb_module /LoadModule socache_shmcb_module /' \
      -e 's/^#LoadModule dav_module /LoadModule dav_module /' \
      -e 's/^#LoadModule dav_fs_module /LoadModule dav_fs_module /' \
      -e 's/^#LoadModule dav_lock_module /LoadModule dav_lock_module /' \
      /usr/local/apache2/conf/httpd.conf \
    && printf '\nInclude conf/extra/webdav.conf\n' \
      >> /usr/local/apache2/conf/httpd.conf \
    && mkdir -p /var/dav-lock \
    && chown www-data:www-data /var/dav-lock

COPY conf/webdav.conf /usr/local/apache2/conf/extra/webdav.conf
