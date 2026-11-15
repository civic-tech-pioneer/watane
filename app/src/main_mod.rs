use crate::CmdError;
use std::{path::PathBuf, sync::Mutex};
use watane_host::{Host, InstalledMod, LocalName, watane_pkg::Mod};

pub(crate) struct MainMod {
    path: PathBuf,
    local: Mutex<Option<String>>,
}
impl MainMod {
    pub fn open(host: &Host) -> Result<Self, CmdError> {
        let path = host.home().join("app/main");
        let local = match std::fs::read_to_string(&path) {
            Ok(text) => {
                let local = text.trim_end_matches('\n');
                if LocalName::parse(local)
                    .ok()
                    .and_then(|n| host.get(&n))
                    .is_some()
                {
                    Some(local.to_owned())
                } else {
                    std::fs::remove_file(&path)?;
                    None
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            path,
            local: Mutex::new(local),
        })
    }
    pub fn get(&self) -> Option<String> {
        self.local.lock().unwrap().clone()
    }
    pub fn startup(&self, host: &Host) -> Option<InstalledMod> {
        let local = LocalName::parse(&self.get()?).ok()?;
        host.get(&local).filter(|m| m.enabled && m.page.is_some())
    }
    pub fn set(&self, host: &Host, local: Option<&str>) -> Result<(), CmdError> {
        if let Some(local) = local {
            let name = LocalName::parse(local)?;
            if host.get(&name).is_none()
                || Mod::read(&host.home().join("mods").join(local))
                    .ok()
                    .and_then(|m| m.page())
                    .is_none()
            {
                return Err(CmdError::new(
                    "not_found",
                    "installed mod with a page not found",
                ));
            }
        }
        let mut current = self.local.lock().unwrap();
        self.write(local)?;
        *current = local.map(str::to_owned);
        Ok(())
    }
    pub fn removed(&self, local: &str) -> Result<(), CmdError> {
        let mut current = self.local.lock().unwrap();
        if current.as_deref() == Some(local) {
            self.write(None)?;
            *current = None;
        }
        Ok(())
    }
    fn write(&self, local: Option<&str>) -> Result<(), CmdError> {
        if let Some(local) = local {
            std::fs::create_dir_all(self.path.parent().unwrap())?;
            std::fs::write(&self.path, format!("{local}\n"))?;
        } else {
            match std::fs::remove_file(&self.path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    pub fn installed(&self, host: &Host, mods: &[InstalledMod]) -> Result<(), CmdError> {
        let mut current = self.local.lock().unwrap();
        if current.is_none() {
            for m in mods {
                if proposes(host, m.local.as_ref()) {
                    self.write(Some(m.local.as_ref()))?;
                    *current = Some(m.local.to_string());
                    break;
                }
            }
        }
        Ok(())
    }
}
pub(crate) fn proposes(host: &Host, local: &str) -> bool {
    Mod::read(&host.home().join("mods").join(local)).is_ok_and(|m| m.main())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }
    async fn install(host: &std::sync::Arc<Host>, paths: Vec<PathBuf>) -> Vec<InstalledMod> {
        let plan = host
            .plan(paths.into_iter().map(watane_host::Source::Folder).collect())
            .unwrap();
        host.install(plan, watane_host::Requester::App)
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn ac1_startup_chooses_enabled_main_page_or_library() {
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Default::default()).unwrap();
        let main = MainMod::open(&host).unwrap();
        assert!(main.startup(&host).is_none());
        let mods = install(&host, vec![fixture("reading-list"), fixture("desk")]).await;
        main.installed(&host, &mods).unwrap();
        let served = host.serve(0, axum::Router::new()).await.unwrap();
        let reopened = MainMod::open(&host).unwrap();
        assert_eq!(reopened.startup(&host).unwrap().local.as_ref(), "desk");
        host.disable(&LocalName::parse("desk").unwrap()).unwrap();
        assert!(reopened.startup(&host).is_none());
        served.shutdown().await;
    }
    #[tokio::test]
    async fn ac2_main_file_choose_unset_and_stale_cleanup() {
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Default::default()).unwrap();
        install(&host, vec![fixture("reading-list")]).await;
        let main = MainMod::open(&host).unwrap();
        main.set(&host, Some("reading-list")).unwrap();
        let path = home.path().join("app/main");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "reading-list\n");
        assert_eq!(
            MainMod::open(&host).unwrap().get().as_deref(),
            Some("reading-list")
        );
        main.set(&host, None).unwrap();
        assert!(!path.exists());
        main.set(&host, Some("reading-list")).unwrap();
        host.remove(&LocalName::parse("reading-list").unwrap(), false)
            .unwrap();
        main.removed("reading-list").unwrap();
        assert!(main.get().is_none());
        assert!(!path.exists());
        assert_eq!(
            main.set(&host, Some("absent")).unwrap_err().code,
            "not_found"
        );
        std::fs::write(&path, "absent\n").unwrap();
        assert!(MainMod::open(&host).unwrap().get().is_none());
        assert!(!path.exists());
    }
    #[tokio::test]
    async fn ac8_first_proposal_in_install_order_and_existing_choice_wins() {
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Default::default()).unwrap();
        let second = tempfile::tempdir().unwrap();
        std::fs::create_dir(second.path().join("wit")).unwrap();
        std::fs::create_dir(second.path().join("ui")).unwrap();
        std::fs::write(
            second.path().join("mod.toml"),
            "name = \"Other desk\"\nsummary = \"Another desk\"\nmain = true\n",
        )
        .unwrap();
        std::fs::write(
            second.path().join("wit/world.wit"),
            "package mods:zdesk@0.1.0; world zdesk {}",
        )
        .unwrap();
        std::fs::write(
            second.path().join("ui/index.html"),
            "<!doctype html><h1>Other desk</h1>",
        )
        .unwrap();
        let installed = install(
            &host,
            vec![
                fixture("desk"),
                second.path().into(),
                fixture("reading-list"),
            ],
        )
        .await;
        let mut ordered: Vec<_> = installed
            .iter()
            .filter(|m| m.local.as_ref() != "reading-list")
            .cloned()
            .collect();
        ordered.sort_by(|a, b| b.local.cmp(&a.local));
        let main = MainMod::open(&host).unwrap();
        main.installed(&host, &ordered).unwrap();
        assert_eq!(main.get().as_deref(), Some("zdesk"));
        main.set(&host, Some("reading-list")).unwrap();
        main.installed(&host, &ordered).unwrap();
        assert_eq!(main.get().as_deref(), Some("reading-list"));
        assert!(!proposes(&host, "absent"));
        std::fs::write(host.home().join("mods/desk/mod.toml"), "invalid").unwrap();
        assert!(!proposes(&host, "desk"));
    }
}
