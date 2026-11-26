//! Fake lane(`screen_scenario.rs`)と GPUI-simulated lane
//! (`scenario_gpui.rs`)で共有する scenario helper。
//! adapter 非依存 — `SemanticGuiTest` port のみを使い、GPUI 型は
//! ここに現れない。
#![allow(dead_code)]

pub mod library;
pub mod mounted;
pub mod scenario;
pub mod tim03_l1;
pub mod vcs;
