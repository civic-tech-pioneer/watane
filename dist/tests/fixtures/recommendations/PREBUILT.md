The mod files are copied from pkg/tests/fixtures/recommendations. The added
logic.wasm is a prebuilt dummy component, generated once with wit-parser
0.254.2 and wit-component 0.254.2 using the same independent producer as
pkg/tests/support/release.rs: Resolve::push_dir, select_world, dummy_module
(Standard32), embed_component_metadata (UTF8), ComponentEncoder (validate).
Tests pack these existing bytes; no compiler or cold build runs in a test.
The added ui/index.html makes the fixture visible without executing logic.
