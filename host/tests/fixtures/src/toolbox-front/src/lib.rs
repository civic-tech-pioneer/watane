wit_bindgen::generate!({ path: "../../mods/toolbox-front/wit", world: "toolbox-front", generate_all });
struct Guest;
impl exports::mods::toolbox_front::api::Guest for Guest {
    fn open(toolsets: Vec<String>) -> Result<String, String> {
        mods::toolbox::api::open(&toolsets)
    }
    fn describe(handle: String) -> Result<Vec<String>, String> {
        mods::toolbox::api::describe(&handle)
    }
    fn call(handle: String, key: String, tool: String, args: String) -> Result<String, String> {
        mods::toolbox::api::call(&handle, &key, &tool, &args)
    }
}
export!(Guest);
