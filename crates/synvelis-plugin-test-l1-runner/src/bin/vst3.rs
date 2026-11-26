//! Test-only VST3 L1 child runner for Synvelis consumer verification.
//! It dlopens the ruviform#630 `ruviform_plugin_test_vst3` cdylib from
//! `SYNVELIS_TIM03_L1_PLUGIN_DIR` or the runner's own directory through
//! the production `Vst3Library` host bridge and serves the scanned
//! factories with `ruviform_plugin_export::run`.

use std::process::ExitCode;

use ruviform_plugin_core::PluginFactory;
use ruviform_plugin_vst3::host::Vst3Library;
use synvelis_plugin_test_l1_runner::fixture_path;

fn main() -> ExitCode {
    let path = match fixture_path("ruviform_plugin_test_vst3", std::env::args_os().skip(1)) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("synvelis-plugin-test-vst3-l1-runner: {error}");
            return ExitCode::from(2);
        }
    };
    let library = match Vst3Library::load(&path) {
        Ok(library) => library,
        Err(error) => {
            eprintln!(
                "synvelis-plugin-test-vst3-l1-runner: cannot load fixture {}: {error}",
                path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    ruviform_plugin_export::run(
        library
            .factories()
            .into_iter()
            .map(|factory| Box::new(factory) as Box<dyn PluginFactory>)
            .collect(),
    )
}
