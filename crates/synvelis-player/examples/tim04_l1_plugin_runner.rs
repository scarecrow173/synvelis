//! TIM-04 test-only L1 composition runner.
//!
//! This executable is spawned only as an out-of-process test child. It loads
//! the real ruviform test fixture libraries through the production VST3/CLAP
//! host bridge loaders and hands the scanned factories to
//! `ruviform_plugin_export::run`. It intentionally accepts no command-line
//! arguments because the upstream wire runner contract is no-argv.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use ruviform_plugin_clap::host::ClapLibrary;
use ruviform_plugin_host::PluginFactory;
use ruviform_plugin_vst3::host::Vst3Library;

fn fixture_file_name(crate_name: &str) -> String {
    format!(
        "{}{}{}",
        env::consts::DLL_PREFIX,
        crate_name,
        env::consts::DLL_SUFFIX
    )
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = env::var_os("SYNVELIS_TIM04_L1_PLUGIN_DIR") {
        dirs.push(PathBuf::from(dir));
    }
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        dirs.push(dir.to_path_buf());
        dirs.push(dir.join("deps"));
    }
    dirs
}

fn resolve_fixture(crate_name: &str) -> Result<PathBuf, String> {
    let file_name = fixture_file_name(crate_name);
    let candidates: Vec<PathBuf> = candidate_dirs()
        .into_iter()
        .map(|dir| dir.join(&file_name))
        .collect();
    if let Some(path) = candidates.iter().find(|path| path.is_file()) {
        return Ok(path.clone());
    }
    let stem = format!("{crate_name}-");
    for dir in candidate_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut matches: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with(&stem) && name.ends_with(env::consts::DLL_SUFFIX)
                    })
            })
            .collect();
        matches.sort();
        if matches.len() == 1 {
            return Ok(matches.remove(0));
        }
    }
    Err(format!(
        "fixture library {file_name} was not found; searched: {}",
        candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn run() -> Result<ExitCode, String> {
    let vst3_path = resolve_fixture("ruviform_plugin_test_vst3")?;
    let clap_path = resolve_fixture("ruviform_plugin_test_clap")?;

    let vst3_library = Vst3Library::load(&vst3_path)
        .map_err(|error| format!("failed to load {}: {error}", vst3_path.display()))?;
    let clap_library = ClapLibrary::load(&clap_path)
        .map_err(|error| format!("failed to load {}: {error}", clap_path.display()))?;

    let mut factories: Vec<Box<dyn PluginFactory>> = vst3_library
        .factories()
        .into_iter()
        .map(|factory| Box::new(factory) as Box<dyn PluginFactory>)
        .collect();
    factories.extend(
        clap_library
            .factories()
            .into_iter()
            .map(|factory| Box::new(factory) as Box<dyn PluginFactory>),
    );

    if factories.is_empty() {
        return Err(format!(
            "no plugin factories were discovered in {} or {}",
            vst3_path.display(),
            clap_path.display()
        ));
    }

    Ok(ruviform_plugin_export::run(factories))
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("tim04-l1-plugin-runner: {error}");
            ExitCode::from(2)
        }
    }
}
