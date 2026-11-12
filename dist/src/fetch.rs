use crate::*;
use tempfile::TempDir;

impl Dist {
    pub(crate) fn unpacked(
        &self,
        package: &Package,
    ) -> Result<(TempDir, watane_pkg::Mod), PackageError> {
        let path = self.home.join("dist/tmp");
        let tmp = TempDir::new_in(&path).map_err(|source| PackageError::Io { path, source })?;
        let m = package.unpack(&tmp.path().join("mod"))?;
        Ok((tmp, m))
    }
    pub(crate) fn cache_path(&self, id: &PackageId) -> PathBuf {
        self.home.join("dist/packages").join(id.to_string())
    }
    pub(crate) fn cache_read(&self, id: &PackageId) -> Result<Vec<u8>, DistError> {
        let path = self.cache_path(id);
        let bytes = site::read_plain(&path)?.ok_or_else(|| DistError::Corrupt {
            path: path.clone(),
            cause: "missing package".into(),
        })?;
        if PackageId::of(&bytes) != *id {
            return Err(DistError::Corrupt {
                path,
                cause: "package hash mismatch".into(),
            });
        }
        Ok(bytes)
    }
    pub(crate) fn cache_write(&self, package: &Package) -> Result<bool, DistError> {
        let path = self.cache_path(&package.id());
        let existed = path
            .try_exists()
            .map_err(|source| site::io(&path, source))?;
        site::write_equal(&path, package.as_bytes(), 0o600)?;
        Ok(!existed)
    }
}

impl Dist {
    pub fn fetch(&self, reference: &ReleaseRef) -> Result<Fetched, DistError> {
        let key = PublicKey::from_bytes(reference.publisher.as_bytes()).map_err(|cause| {
            DistError::BadKey {
                key: reference.publisher.clone(),
                cause,
            }
        })?;
        let view = self.channel(&key)?;
        if let Status::Compromised { seq } = view.status {
            return Err(DistError::Compromised { channel: key, seq });
        }
        let info = view
            .releases
            .into_iter()
            .find(|r| r.reference == *reference)
            .ok_or_else(|| DistError::UnknownRelease {
                release: reference.clone(),
            })?;
        if let Some(withdrawn) = &info.withdrawn {
            return Err(DistError::Withdrawn {
                release: reference.clone(),
                reason: withdrawn.reason.clone(),
            });
        }
        let package = self.fetch_package(&info, view.base.as_deref().filter(|_| view.followed))?;
        let entry = self.log.entries(&key, info.seq, 1)?.remove(0);
        Ok(Fetched {
            package,
            entry,
            info,
            publisher_name: view.name,
            checked_at: view.last_success,
        })
    }
    pub(crate) fn fetch_package(
        &self,
        info: &ReleaseInfo,
        base: Option<&str>,
    ) -> Result<Package, DistError> {
        let id = &info.release.package;
        let path = self.cache_path(id);
        let cached = site::read_plain(&path)?;
        let bytes = match cached {
            Some(bytes) if PackageId::of(&bytes) == *id => Some(bytes),
            Some(_) => {
                std::fs::remove_file(&path).map_err(|e| site::io(&path, e))?;
                None
            }
            None => None,
        };
        let url = base
            .map(|b| http::file_url(b, &format!("packages/{id}")))
            .unwrap_or_else(|| format!("cache:{id}"));
        let bytes = match bytes {
            Some(bytes) => bytes,
            None => {
                if base.is_none() {
                    return Err(DistError::NotFollowed {
                        channel: info.channel,
                    });
                }
                self.get(
                    &url,
                    info.release.size.min(self.options.max_package),
                    self.options.package_timeout,
                    false,
                )?
            }
        };
        let found = PackageId::of(&bytes);
        if found != *id {
            return Err(DistError::HashMismatch {
                url,
                expected: id.clone(),
                found,
            });
        }
        let package = Package::from_bytes(bytes).map_err(|cause| DistError::BadPackage {
            url: url.clone(),
            cause,
        })?;
        let (_tmp, m) = self
            .unpacked(&package)
            .map_err(|cause| DistError::BadPackage {
                url: url.clone(),
                cause,
            })?;
        info.release
            .check(&m, &package)
            .map_err(|cause| DistError::PackageMismatch { url, cause })?;
        self.cache_write(&package)?;
        Ok(package)
    }
    pub fn open_link(&self, link: &Link) -> Result<Fetched, DistError> {
        let slug = link.slug().ok_or(DistError::BadLink {
            cause: "a release link needs a slug",
        })?;
        if link.channel() != self.identity() {
            let followed = self
                .follows
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains_key(&link.channel());
            if followed {
                self.refresh_channel(&link.channel())?;
            } else {
                self.follow(link)?;
            }
        }
        let info = self
            .latest(&link.channel(), slug)?
            .ok_or_else(|| DistError::NoRelease {
                channel: link.channel(),
                slug: slug.clone(),
            })?;
        self.fetch(&info.reference)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoSign;
    impl Confirm for NoSign {
        fn confirm(&self, _: &SignRequest) -> Decision {
            panic!("fetch must not sign")
        }
    }
    #[test]
    fn ac1_fetch_checks_release_against_package() {
        let home = tempfile::tempdir().unwrap();
        let dist = Dist::open(home.path(), Arc::new(NoSign), Options::default()).unwrap();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading-list");
        let package = watane_pkg::pack(&dir, false).unwrap();
        let m = watane_pkg::Mod::read(&dir).unwrap();
        let mut release = Release::new(&m, &package).unwrap();
        release.name = "Does not match the package".into();
        // Preload valid package bytes; even a valid hash cannot replace Release::check.
        std::fs::write(dist.cache_path(&package.id()), package.as_bytes()).unwrap();
        let info = ReleaseInfo {
            reference: ReleaseRef {
                publisher: PublisherKey::from_bytes(*dist.identity().as_bytes()),
                slug: release.slug.clone(),
                version: release.version.clone(),
            },
            channel: dist.identity(),
            entry: "0000000000000000000000000000000000000000000000000000000000000000"
                .parse()
                .unwrap(),
            seq: 0,
            release,
            withdrawn: None,
        };
        assert!(matches!(
            dist.fetch_package(&info, None),
            Err(DistError::PackageMismatch {
                cause: ReleaseError::Mismatch { field: "name" },
                ..
            })
        ));
    }
}
