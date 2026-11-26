//! Shared argv and fixture-artifact resolution for Synvelis's test-only
//! TIM-03 L1 runner binaries.
//!
//! This crate is not part of the product runtime. Its two executables
//! exist so consumer tests can reproducibly spawn the ruviform#630 real
//! ABI fixture libraries through `ruviform-plugin-export`'s L1 wire
//! runtime without relying on upstream runner binaries being propagated
//! through a library dependency edge.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

/// Optional directory fallback used before the runner executable's own
/// directory.
pub const PLUGIN_DIR_ENV: &str = "SYNVELIS_TIM03_L1_PLUGIN_DIR";

/// The platform-specific fixture library file name for `stem`.
#[must_use]
pub fn fixture_library_name(stem: &str) -> String {
    format!(
        "{}{}{}",
        env::consts::DLL_PREFIX,
        stem,
        env::consts::DLL_SUFFIX
    )
}

/// Resolves the fixture cdylib for `stem` from `args` (normally
/// `env::args_os().skip(1)`). `ruviform_plugin_export::run` owns the
/// process argv contract and accepts no arguments, so fixture discovery
/// is environmental rather than argv-based: `PLUGIN_DIR_ENV` first and
/// then the runner executable's own directory under the conventional
/// un-hashed cdylib name. Any argv entry is a usage rejection.
///
/// # Errors
///
/// Returns a human-readable usage or resolution error. The caller emits
/// it and exits nonzero before the L1 handshake.
pub fn fixture_path(
    stem: &str,
    args: impl IntoIterator<Item = OsString>,
) -> Result<PathBuf, String> {
    let mut args = args.into_iter();
    if let Some(arg) = args.next() {
        return Err(format!(
            "unsupported runner argument {}; usage: <runner>",
            arg.to_string_lossy()
        ));
    }
    let name = fixture_library_name(stem);
    if let Some(dir) = env::var_os(PLUGIN_DIR_ENV) {
        return Ok(PathBuf::from(dir).join(name));
    }
    let exe = env::current_exe().map_err(|error| format!("current_exe failed: {error}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "runner executable has no parent directory".to_owned())?;
    Ok(dir.join(name))
}
