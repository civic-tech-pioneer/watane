wit_bindgen::generate!({ path: "../../mods/item-bad/wit", world: "item-bad", generate_all });
struct Guest;
impl exports::mods::menu::menu::Guest for Guest {
    fn ask() -> Vec<String> {
        watane::host::state::put("key", b"1");
        let mut n = 0u64;
        for i in 0..2_000_000u64 {
            n = std::hint::black_box(n.wrapping_add(i));
        }
        std::hint::black_box(n);
        panic!("bad answer");
    }
    fn clicked(_id: String) {}
}
export!(Guest);
