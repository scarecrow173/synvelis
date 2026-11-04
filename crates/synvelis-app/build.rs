//! Bakes the pinned ruviform revision (from `Cargo.lock`) into the
//! binary as `SYNVELIS_RUVIFORM_REV`, so `revision::check_bundle` can
//! verify a frontend bundle was built against the same ruviform commit
//! the host is linked with.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let lock = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("Cargo.lock");
    let rev = fs::read_to_string(&lock)
        .ok()
        .and_then(|content| ruviform_rev(&content))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=SYNVELIS_RUVIFORM_REV={rev}");
    println!("cargo:rerun-if-changed={}", lock.display());
}

fn ruviform_rev(lock: &str) -> Option<String> {
    for block in lock.split("[[package]]").skip(1) {
        let is_ruviform_ui = block.lines().any(|l| {
            l.trim()
                .strip_prefix("name = ")
                .is_some_and(|n| n.trim_matches('"') == "ruviform-ui")
        });
        if !is_ruviform_ui {
            continue;
        }
        let source = block
            .lines()
            .find_map(|l| l.trim().strip_prefix("source = "))?;
        return source
            .trim_matches('"')
            .rsplit('#')
            .next()
            .map(str::to_string);
    }
    None
}
