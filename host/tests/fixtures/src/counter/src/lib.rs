wit_bindgen::generate!({ path: "../../mods/counter/wit", world: "counter", generate_all });
struct Counter;
impl exports::mods::counter::api::Guest for Counter {
    fn get() -> u32 {
        watane::host::state::get("count")
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
            .unwrap_or(0)
    }
    fn add() -> u32 {
        let count = Self::get() + 1;
        watane::host::state::put("count", &count.to_le_bytes());
        mods::counter::changes::counted(count);
        count
    }
    fn add_then_fail() {
        Self::add();
        panic!("deliberate failure");
    }
    fn who() -> String {
        watane::host::caller::who()
    }
}
export!(Counter);
