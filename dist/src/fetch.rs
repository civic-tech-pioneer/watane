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
