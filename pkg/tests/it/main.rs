//! pkg's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[path = "../support/release.rs"]
mod support;

mod add_provider;
mod component;
mod deps_json;
mod folder;
mod grant_manage;
mod grants;
mod mod_toml_main;
mod mod_toml_repository;
mod names;
mod outlet_functions;
mod package;
mod release_check;
mod release_golden;
mod release_refusals;
mod s22;
mod s46;
mod s49;
mod s57;
mod s91;
mod unpack_umask;
mod validation;
