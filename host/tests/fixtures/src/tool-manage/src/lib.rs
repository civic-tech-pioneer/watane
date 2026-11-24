wit_bindgen::generate!({ path: "../../mods/tool-manage/wit", world: "tool", generate_all });
struct Guest;
impl exports::mods::tool::api::Guest for Guest {
    fn hello(name: String) -> String {
        format!("hello {name}")
    }
    fn double(n: u32) -> u32 {
        n * 2
    }
    fn who() -> String {
        watane::host::caller::identity()
    }
    fn slow(ms: u32) -> String {
        std::thread::sleep(std::time::Duration::from_millis(u64::from(ms)));
        "slept".into()
    }
    fn refuse() -> Result<String, String> {
        Err("refused".into())
    }
    fn boom() {
        let _ = watane::host::manage::mods();
        panic!("boom");
    }
}
export!(Guest);
