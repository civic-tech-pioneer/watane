wit_bindgen::generate!({ path: "../../mods/manager/wit", world: "manager", generate_all });
use exports::mods::manager::api::Guest;
use watane::host::manage as m;
struct Component;
impl Guest for Component {
    fn mods() -> Vec<m::ModInfo> {
        m::mods()
    }
    fn disable(local: String) -> Result<(), m::Failure> {
        m::disable(&local)
    }
    fn withdraw(local: String, grant: m::Grant) -> Result<(), m::Failure> {
        m::withdraw(&local, &grant)
    }
    fn close_page(local: String) -> Result<(), m::Failure> {
        m::close_page(&local)
    }
    fn open_page(local: String) -> Result<(), m::Failure> {
        m::open_page(&local)
    }
    fn open_page_then_fail(local: String) {
        m::open_page(&local).unwrap();
        panic!("open-page-then-fail");
    }
    fn close_open_close(local: String) {
        m::close_page(&local).unwrap();
        m::open_page(&local).unwrap();
        m::close_page(&local).unwrap();
    }
    fn request_install(release: String, link: Option<String>) -> Result<String, m::Failure> {
        m::request_install(&release, link.as_deref())
    }
    fn request_update(local: String) -> Result<String, m::Failure> {
        m::request_update(&local)
    }
    fn request_enable(local: String) -> Result<String, m::Failure> {
        m::request_enable(&local)
    }
    fn request_grant(local: String, grant: m::Grant) -> Result<String, m::Failure> {
        m::request_grant(&local, &grant)
    }
    fn request_remove(local: String, delete_data: bool) -> Result<String, m::Failure> {
        m::request_remove(&local, delete_data)
    }
    fn request(id: String) -> Option<m::RequestState> {
        m::request(&id)
    }
    fn disable_then_fail(local: String) {
        let _ = m::disable(&local);
        panic!("disable-then-fail");
    }
    fn disable_then_read(local: String) -> bool {
        let _ = m::disable(&local);
        m::mods().iter().any(|i| i.local == local && i.enabled)
    }
    fn who() -> String {
        watane::host::caller::who()
    }
}
export!(Component);
