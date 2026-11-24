//! synvelis#633 — docker WebDAV 実 provider 接続 probe。
//!
//! `docker-compose.webdav.yml` で立てた TLS・Basic 認証付き WebDAV
//! サーバへ、本物の `synvelis_library::webdav::WebDavProvider`
//! （`WebDavUrl::parse` + `WebDavConfig` + `TlsTrust::from_pem` +
//! `CredentialResolver`）を通して接続し、capabilities / browse /
//! stat / pin / open_pinned / open_preview を実走する。
//!
//! 使い方（repo ルートから）:
//! ```sh
//! set -a && . docker/.env && set +a   # WEBDAV_USER/WEBDAV_PASSWORD
//! cargo run --manifest-path docker/webdav-probe/Cargo.toml
//! ```
//! 環境変数（docker/.env を source して供給する）:
//!   WEBDAV_URL      default https://localhost:8443
//!   WEBDAV_ROOT     default /library
//!   WEBDAV_CA       default docker/webdav/certs/ca.crt
//!   WEBDAV_USER     required（Basic 認証ユーザー）
//!   WEBDAV_PASSWORD required（Basic 認証パスワード）
//!   WEBDAV_FOREIGN_CA default docker/webdav/certs/ca-foreign.crt
//!                   （TLS negative check 用の無関係な CA）
//!
//! この crate は `mise.toml` の検証 lane ではなく、docker/ 配下の
//! 補助 lane（verify.sh）から実行する。root workspace からは
//! `[workspace]` 宣言で独立している。

use std::env;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::sync::Arc;

use base64::Engine as _;
use synvelis_library::{
    AssetProvider, BrowseRequest, BrowseScope, ByteRange, CredentialKey, CredentialResolver,
    EntryKind, FilterSpec, OpaqueCredential, PinProof, PreconditionKind, PrincipalId,
    ProviderContext, ProviderError, ProviderId, RemoteAssetRef, SortSpec, StrongRevisionToken,
    TlsTrust, WebDavConfig, WebDavProvider, WebDavUrl,
};

/// env で与えた `Authorization` header 値をそのまま返す最小の
/// `CredentialResolver`。アプリ層の OS credential store とは別に、
/// probe は env 由来の固定値を key に束ねるだけでよい。
struct StaticResolver(String);

impl CredentialResolver for StaticResolver {
    fn resolve(&self, key: &CredentialKey) -> Result<OpaqueCredential, ProviderError> {
        Ok(OpaqueCredential::new(key.clone(), self.0.clone()))
    }
}

fn required_env(name: &str) -> Result<String, Box<dyn Error>> {
    env::var(name).map_err(|_| {
        format!("{name} is not set — `set -a && . docker/.env && set +a` してから実行する").into()
    })
}

fn run() -> Result<(), Box<dyn Error>> {
    let url_text = env::var("WEBDAV_URL").unwrap_or_else(|_| "https://localhost:8443".to_owned());
    let root = env::var("WEBDAV_ROOT").unwrap_or_else(|_| "/library".to_owned());
    let ca_path = env::var("WEBDAV_CA").unwrap_or_else(|_| "docker/webdav/certs/ca.crt".to_owned());
    let user = required_env("WEBDAV_USER")?;
    let password = required_env("WEBDAV_PASSWORD")?;

    let ca_pem = fs::read(&ca_path)
        .map_err(|e| format!("cannot read CA pem {ca_path}: {e} — compose up を先に実行する"))?;
    let trust = TlsTrust::from_pem(&ca_pem)?;

    let mut config = WebDavConfig::new(WebDavUrl::parse(&url_text)?);
    config.root_path = root.clone();
    config.credential_key = Some(CredentialKey::new("synvelis-webdav-docker")?);
    let provider = WebDavProvider::new(ProviderId::new(0x633), config)?;

    let authorization = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
    );
    let mut cx = ProviderContext::new(PrincipalId::new("webdav-probe")?);
    cx.tls = Some(trust);
    cx.credentials = Some(Arc::new(StaticResolver(authorization.clone())));

    println!("[probe] url={url_text} root={root} ca={ca_path}");

    let caps = provider.capabilities(None);
    println!(
        "[probe] capabilities: browse={:?} stat={:?} pin={:?} range_read={:?} preview={:?} access={:?}",
        caps.browse, caps.stat, caps.pin, caps.range_read, caps.preview, caps.access
    );

    let req = BrowseRequest {
        scope: BrowseScope::Root,
        page_limit: 64,
        cursor: None,
        sort: SortSpec::default(),
        filter: FilterSpec::default(),
    };
    let page = provider.browse(&req, &cx)?;
    println!(
        "[probe] browse root: {} entries coverage={:?}",
        page.entries.len(),
        page.coverage
    );
    for e in &page.entries {
        println!("[probe]   {:?} {} ({})", e.kind, e.display_name, e.locator);
    }

    let asset = page
        .entries
        .iter()
        .find(|e| e.kind == EntryKind::Asset)
        .ok_or("browse が Asset entry を返さなかった — fixture data を確認")?;
    let aref = RemoteAssetRef {
        provider: provider.id(),
        locator: asset.locator.clone(),
    };

    let stat = provider.stat(&aref, &cx)?;
    println!(
        "[probe] stat {}: availability={:?} byte_len={:?}",
        aref.locator, stat.availability, stat.facts.byte_len
    );

    let pin = provider.pin(&aref, &cx)?;
    let validator = match &pin.proof {
        PinProof::ConditionalStrong {
            token:
                StrongRevisionToken::StrongValidator {
                    validator,
                    precondition,
                },
        } => {
            if *precondition != PreconditionKind::IfMatch {
                return Err(format!("unexpected precondition {precondition:?}").into());
            }
            validator.clone()
        }
        other => {
            return Err(format!(
                "webdav pin must be ConditionalStrong/StrongValidator, got {other:?}"
            )
            .into());
        }
    };
    println!(
        "[probe] pin: validator={validator} precondition=IfMatch expected_size={:?}",
        pin.expected_size
    );

    let mut pr = provider.open_pinned(&pin, &cx)?;
    let mut body = Vec::new();
    pr.reader.read_to_end(&mut body)?;
    let receipt = pr
        .completion
        .take()
        .ok_or("open_pinned stream did not finish")??;
    println!(
        "[probe] open_pinned: {} bytes read, verified={} policy={:?}",
        receipt.bytes_read, receipt.verified, pr.claims.verification
    );

    let mut pv = provider.open_preview(&aref, Some(ByteRange { offset: 0, len: 16 }), &cx)?;
    let mut preview = Vec::new();
    pv.reader.read_to_end(&mut preview)?;
    let pv_receipt = pv
        .completion
        .take()
        .ok_or("open_preview stream did not finish")??;
    println!(
        "[probe] open_preview range 0..16: {} bytes",
        pv_receipt.bytes_read
    );

    // negative: 誤った credential は server が 401 で拒否し、
    // provider は認証系の kind で Err を返すはず。
    let mut bad_cx = ProviderContext::new(PrincipalId::new("webdav-probe")?);
    bad_cx.tls = Some(TlsTrust::from_pem(&ca_pem)?);
    bad_cx.credentials = Some(Arc::new(StaticResolver(format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}-wrong"))
    ))));
    match provider.browse(&req, &bad_cx) {
        Ok(_) => return Err("誤った credential が accept された".into()),
        Err(e) => println!("[probe] bad credential rejected: kind={:?}", e.kind()),
    }

    // negative: サーバ証明書の署名 CA と無関係な foreign CA を
    // anchor にした TLS は検証失敗（TlsVerification）するはず。
    let foreign_path = env::var("WEBDAV_FOREIGN_CA")
        .unwrap_or_else(|_| "docker/webdav/certs/ca-foreign.crt".to_owned());
    let foreign_pem = fs::read(&foreign_path)
        .map_err(|e| format!("cannot read foreign CA pem {foreign_path}: {e}"))?;
    let mut foreign_cx = ProviderContext::new(PrincipalId::new("webdav-probe")?);
    foreign_cx.tls = Some(TlsTrust::from_pem(&foreign_pem)?);
    foreign_cx.credentials = Some(Arc::new(StaticResolver(authorization)));
    match provider.browse(&req, &foreign_cx) {
        Ok(_) => return Err("foreign CA anchor が accept された".into()),
        Err(e) => println!("[probe] foreign CA rejected: kind={:?}", e.kind()),
    }

    println!("PROBE PASS");
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("PROBE FAIL: {e}");
        std::process::exit(1);
    }
}
