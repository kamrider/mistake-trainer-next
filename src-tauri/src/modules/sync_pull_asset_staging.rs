use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use super::SyncPullError;
use crate::modules::sync_store::WireAsset;

#[derive(Debug)]
pub(super) struct StagedAsset {
    asset: WireAsset,
    relative_path: String,
    staged_path: PathBuf,
    final_path: PathBuf,
    moved_to_final: bool,
}

pub(super) fn stage_encrypted_asset(
    blob_root: &Path,
    asset: &WireAsset,
    page_id: &str,
    encrypted: &[u8],
) -> Result<StagedAsset, SyncPullError> {
    let shard = &asset.id[..2];
    let relative_path = format!("blobs/{shard}/{}.mtb", asset.id);
    let final_path = blob_root.join(&relative_path);
    let staged_root = blob_root.join(".sync-pull").join(page_id);
    fs::create_dir_all(&staged_root)?;
    let staged_path = staged_root.join(format!("{}.mtb", asset.id));
    if let Err(error) = fs::write(&staged_path, encrypted) {
        let _ = fs::remove_file(&staged_path);
        let _ = fs::remove_dir(&staged_root);
        return Err(error.into());
    }
    Ok(StagedAsset {
        asset: asset.clone(),
        relative_path,
        staged_path,
        final_path,
        moved_to_final: false,
    })
}

impl StagedAsset {
    pub(super) fn asset(&self) -> &WireAsset {
        &self.asset
    }

    pub(super) fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub(super) fn promote(&mut self) -> Result<(), SyncPullError> {
        if let Some(parent) = self.final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if self.final_path.exists() {
            return Err(SyncPullError::AssetMismatch);
        }
        fs::rename(&self.staged_path, &self.final_path)?;
        self.moved_to_final = true;
        Ok(())
    }
}

pub(super) fn cleanup_page(staged: &[StagedAsset], rollback_final: bool) {
    let mut roots = HashSet::new();
    for asset in staged {
        let _ = fs::remove_file(&asset.staged_path);
        if let Some(root) = asset.staged_path.parent() {
            roots.insert(root.to_owned());
        }
        if rollback_final && asset.moved_to_final {
            let _ = fs::remove_file(&asset.final_path);
        }
    }
    for root in roots {
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollback_preserves_a_preexisting_unowned_final_file() {
        let root = tempfile::tempdir().expect("root");
        let asset = asset("0191365e-2f2f-7b89-b3b0-111111111111");
        let final_path = root
            .path()
            .join("blobs/01")
            .join(format!("{}.mtb", asset.id));
        fs::create_dir_all(final_path.parent().expect("parent")).expect("final parent");
        fs::write(&final_path, b"preexisting").expect("preexisting final");
        let mut staged =
            stage_encrypted_asset(root.path(), &asset, "page-1", b"downloaded").expect("stage");

        assert!(matches!(
            staged.promote(),
            Err(SyncPullError::AssetMismatch)
        ));
        cleanup_page(std::slice::from_ref(&staged), true);

        assert_eq!(
            fs::read(final_path).expect("preserved final"),
            b"preexisting"
        );
    }

    #[test]
    fn rollback_removes_a_final_file_promoted_by_this_page() {
        let root = tempfile::tempdir().expect("root");
        let asset = asset("0191365e-2f2f-7b89-b3b0-222222222222");
        let mut staged =
            stage_encrypted_asset(root.path(), &asset, "page-2", b"downloaded").expect("stage");
        let final_path = staged.final_path.clone();
        let staged_root = staged.staged_path.parent().expect("staged root").to_owned();

        staged.promote().expect("promote");
        cleanup_page(std::slice::from_ref(&staged), true);

        assert!(!final_path.exists());
        assert!(!staged_root.exists());
    }

    #[test]
    fn success_cleanup_keeps_the_promoted_final_file() {
        let root = tempfile::tempdir().expect("root");
        let asset = asset("0191365e-2f2f-7b89-b3b0-333333333333");
        let mut staged =
            stage_encrypted_asset(root.path(), &asset, "page-3", b"downloaded").expect("stage");
        let final_path = staged.final_path.clone();
        let staged_root = staged.staged_path.parent().expect("staged root").to_owned();

        staged.promote().expect("promote");
        cleanup_page(std::slice::from_ref(&staged), false);

        assert_eq!(fs::read(final_path).expect("final"), b"downloaded");
        assert!(!staged_root.exists());
    }

    fn asset(id: &str) -> WireAsset {
        WireAsset {
            id: id.to_owned(),
            plaintext_sha256: "a".repeat(64),
            storage_object: "remote/object".to_owned(),
            byte_length: 10,
            media_type: "image/png".to_owned(),
            revision: 1,
            created_at_utc_ms: 10,
        }
    }
}
