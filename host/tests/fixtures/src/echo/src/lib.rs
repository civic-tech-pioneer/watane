wit_bindgen::generate!({ path: "../../mods/echo/wit", world: "echo", generate_all });
use exports::mods::echo::api::*;
struct Echo;
impl Guest for Echo {
    fn id_bool(x: bool) -> bool {
        x
    }
    fn id_u8(x: u8) -> u8 {
        x
    }
    fn id_u16(x: u16) -> u16 {
        x
    }
    fn id_u32(x: u32) -> u32 {
        x
    }
    fn id_u64(x: u64) -> u64 {
        x
    }
    fn id_s8(x: i8) -> i8 {
        x
    }
    fn id_s16(x: i16) -> i16 {
        x
    }
    fn id_s32(x: i32) -> i32 {
        x
    }
    fn id_s64(x: i64) -> i64 {
        x
    }
    fn id_f32(x: f32) -> f32 {
        x
    }
    fn id_f64(x: f64) -> f64 {
        x
    }
    fn id_char(x: char) -> char {
        x
    }
    fn id_string(x: String) -> String {
        x
    }
    fn id_list(x: Vec<u8>) -> Vec<u8> {
        x
    }
    fn id_tuple(x: (u8, String)) -> (u8, String) {
        x
    }
    fn id_record(x: Entry) -> Entry {
        x
    }
    fn id_variant(x: Choice) -> Choice {
        x
    }
    fn id_enum(x: Status) -> Status {
        x
    }
    fn id_flags(x: Bits) -> Bits {
        x
    }
    fn id_option(x: Option<u32>) -> Option<u32> {
        x
    }
    fn id_result(x: Result<u32, String>) -> Result<u32, String> {
        x
    }
    fn id_unit_result(x: Result<(), ()>) -> Result<(), ()> {
        x
    }
    fn opt_opt(_: Option<Option<u32>>) {}
    fn put(key: String, value: Vec<u8>) {
        watane::host::state::put(&key, &value);
    }
    fn read(key: String) -> Option<Vec<u8>> {
        watane::host::state::get(&key)
    }
    fn delete(key: String) {
        watane::host::state::delete(&key);
    }
    fn put_twice() {
        Self::put("a".into(), vec![1, 2]);
        Self::put("b".into(), vec![3, 4]);
    }
    fn nan() -> f64 {
        f64::NAN
    }
    fn infinity() -> f32 {
        f32::INFINITY
    }
}
export!(Echo);
