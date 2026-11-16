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
    pub fn proposed(
        &self,
        host: &Host,
        entries: &[watane_host::PlanEntry],
    ) -> Result<Option<String>, CmdError> {
        if self.get().is_some() || entries.is_empty() {
            return Ok(None);
        }
        // PlanEntry omits `main`. Read pkg metadata from the host's documented
        // staging layout (host 4.2); match bytes, never just a slug/version that
        // another pending plan might share. Source files are kept when present.
        let mut proposals = std::collections::HashSet::new();
        for stage in std::fs::read_dir(host.home().join("staging"))?.flatten() {
            let folder = stage.path().join("mod");
            let Ok(metadata) = Mod::read(&folder) else {
                continue;
            };
            // Another plan's staging may vanish mid-read; skip it, never deny.
            if metadata.main()
                && let Ok(package) = watane_host::watane_pkg::pack(&folder, true)
            {
                proposals.insert(package.id());
            }
        }
        Ok(entries
            .iter()
            .find(|e| {
                matches!(e.change, watane_host::Change::New) && proposals.contains(&e.package)
            })
            .map(|e| e.local.to_string()))
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
    pub fn adopt(&self, local: &str) -> Result<(), CmdError> {
        let mut current = self.local.lock().unwrap();
        if current.is_none() {
            self.write(Some(local))?;
            *current = Some(local.to_owned());
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
    async fn ac2_s81_consent_selects_first_proposal_only_without_a_main() {
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Default::default()).unwrap();
        install(&host, vec![fixture("reading-list")]).await;
        let main = MainMod::open(&host).unwrap();
        let plan = host
            .plan(vec![watane_host::Source::Folder(fixture("desk"))])
            .unwrap();
        assert_eq!(
            main.proposed(&host, plan.entries()).unwrap().as_deref(),
            Some("desk")
        );
        main.set(&host, Some("reading-list")).unwrap();
        assert_eq!(main.proposed(&host, plan.entries()).unwrap(), None);
        main.set(&host, None).unwrap();
        let other = host
            .plan(vec![watane_host::Source::Folder(fixture("reading-list"))])
            .unwrap();
        assert_eq!(main.proposed(&host, other.entries()).unwrap(), None);
        // Another pending plan with the same slug/version must not borrow
        // Desk's proposal. This also exercises packages containing source.
        let variant = tempfile::tempdir().unwrap();
        std::fs::create_dir(variant.path().join("wit")).unwrap();
        std::fs::create_dir(variant.path().join("ui")).unwrap();
        for file in ["wit/world.wit", "ui/index.html"] {
            std::fs::copy(fixture("desk").join(file), variant.path().join(file)).unwrap();
        }
        std::fs::write(
            variant.path().join("mod.toml"),
            "name = \"Desk\"\nsummary = \"Desk\"\nmain = false\n",
        )
        .unwrap();
        let nonmain = host
            .plan(vec![watane_host::Source::Folder(variant.path().into())])
            .unwrap();
        assert_eq!(main.proposed(&host, nonmain.entries()).unwrap(), None);
        std::fs::write(
            variant.path().join("mod.toml"),
            "name = \"Other desk\"\nsummary = \"Desk\"\nmain = true\n",
        )
        .unwrap();
        std::fs::write(
            variant.path().join("wit/world.wit"),
            "package mods:zdesk@0.1.0; world zdesk {}",
        )
        .unwrap();
        std::fs::create_dir(variant.path().join("tests")).unwrap();
        std::fs::write(variant.path().join("tests/example.json"), "{}").unwrap();
        let pkg = watane_host::watane_pkg::pack(variant.path(), true).unwrap();
        // A folder plan repacks without source; put the source-bearing bytes
        // in an independent staged directory to check the adapter's id match.
        let staged = host.home().join("staging/source-package");
        std::fs::create_dir(&staged).unwrap();
        pkg.unpack(&staged.join("mod")).unwrap();
        let mut second = plan.entries()[0].clone();
        second.package = pkg.id();
        second.local = LocalName::parse("zdesk").unwrap();
        let ordered = [second, plan.entries()[0].clone()];
        assert_eq!(
            main.proposed(&host, &ordered).unwrap().as_deref(),
            Some("zdesk")
        );
        let reversed = ordered.into_iter().rev().collect::<Vec<_>>();
        assert_eq!(
            main.proposed(&host, &reversed).unwrap().as_deref(),
            Some("desk")
        );
    }
    #[tokio::test]
    async fn ac1_startup_chooses_enabled_main_page_or_library() {
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Default::default()).unwrap();
        let main = MainMod::open(&host).unwrap();
        assert!(main.startup(&host).is_none());
        install(&host, vec![fixture("reading-list"), fixture("desk")]).await;
        main.set(&host, Some("desk")).unwrap();
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
}
