wit_bindgen::generate!({ path: "../../mods/hog/wit", world: "hog", generate_all });
struct Hog;
static COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
impl exports::mods::hog::api::Guest for Hog {
    fn spin() {
        loop {
            std::hint::spin_loop();
        }
    }
    fn grow(pages: u32) {
        core::arch::wasm32::memory_grow::<0>(pages as usize);
    }
    fn ping() -> String {
        "alive".into()
    }
    fn fresh() -> u32 {
        COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
    fn probe() -> Vec<String> {
        vec![
            if std::fs::read_dir("/").is_err() {
                "no-dir"
            } else {
                "directory"
            }
            .into(),
            if std::env::var_os("HOME").is_none() && std::env::args().count() == 0 {
                "no-env"
            } else {
                "environment"
            }
            .into(),
            if std::net::TcpStream::connect("127.0.0.1:9").is_err() {
                "no-socket"
            } else {
                "socket"
            }
            .into(),
        ]
    }
}
export!(Hog);
