These three dummy components match the WIT variants built in support::interface_fixture.
They were generated once using wit-parser and wit-component 0.254.2: push_dir,
select_world, dummy_module(Standard32), embed_component_metadata(UTF8),
ComponentEncoder(validate=true). The interfaces are count() -> u32 and ping().
The existing page-only fixture has no component; recommendations uses different
interfaces and cannot serve as these variants. Tests copy the page fixture to a
temp directory and use these prebuilt bytes, without a compiler or cold build.
