use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

use super::capture_inbox::{CaptureInboxError, MAX_ENCRYPTED_CAPTURE_BYTES};

#[derive(Debug)]
pub(crate) struct StagedCaptureAsset {
    asset_id: String,
    relative_path: String,
    staged_path: PathBuf,
    final_path: PathBuf,
    moved_to_final: bool,
    committed: bool,
}

pub(crate) fn stage_encrypted_capture_asset(
    blob_root: &Path,
    asset_id: String,
    encrypted: &[u8],
) -> Result<StagedCaptureAsset, CaptureInboxError> {
    let shard = asset_id.get(..2).ok_or(CaptureInboxError::InvalidInput)?;
    let relative = PathBuf::from("blobs")
        .join(shard)
        .join(format!("{asset_id}.mtb"));
    let staging_root = blob_root.join(".staging");
    fs::create_dir_all(&staging_root)?;
    let staged_path = staging_root.join(format!("{asset_id}.capture.tmp"));
    if let Err(error) = fs::write(&staged_path, encrypted) {
        let _ = fs::remove_file(&staged_path);
        return Err(error.into());
    }
    Ok(StagedCaptureAsset {
        asset_id,
        relative_path: relative.to_string_lossy().replace('\\', "/"),
        staged_path,
        final_path: blob_root.join(relative),
        moved_to_final: false,
        committed: false,
    })
}

impl StagedCaptureAsset {
    pub(crate) fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub(crate) fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub(crate) fn promote(&mut self) -> Result<(), CaptureInboxError> {
        if let Some(parent) = self.final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if self.final_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "capture asset target already exists",
            )
            .into());
        }
        fs::rename(&self.staged_path, &self.final_path)?;
        self.moved_to_final = true;
        Ok(())
    }

    pub(crate) fn mark_committed(&mut self) {
        debug_assert!(self.moved_to_final);
        self.committed = true;
    }
}

impl Drop for StagedCaptureAsset {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.staged_path);
        if self.moved_to_final && !self.committed {
            let _ = fs::remove_file(&self.final_path);
        }
    }
}

pub(crate) fn validate_relative_asset_path(
    encrypted_path: &str,
) -> Result<&Path, CaptureInboxError> {
    let relative = Path::new(encrypted_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CaptureInboxError::InvalidAssetPath);
    }
    Ok(relative)
}

pub(crate) fn remove_encrypted_blob(
    blob_root: &Path,
    encrypted_path: &str,
) -> Result<(), CaptureInboxError> {
    let path = blob_root.join(validate_relative_asset_path(encrypted_path)?);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn read_encrypted_blob(
    blob_root: &Path,
    encrypted_path: &str,
) -> Result<Vec<u8>, CaptureInboxError> {
    let file = std::fs::File::open(blob_root.join(validate_relative_asset_path(encrypted_path)?))?;
    let mut reader = file.take(MAX_ENCRYPTED_CAPTURE_BYTES + 1);
    let mut encrypted = Vec::new();
    reader.read_to_end(&mut encrypted)?;
    if u64::try_from(encrypted.len()).unwrap_or(u64::MAX) > MAX_ENCRYPTED_CAPTURE_BYTES {
        return Err(CaptureInboxError::InvalidImage);
    }
    Ok(encrypted)
}

pub(crate) fn image_format_for_media_type(
    media_type: &str,
) -> Result<image::ImageFormat, CaptureInboxError> {
    match media_type {
        "image/png" => Ok(image::ImageFormat::Png),
        "image/jpeg" => Ok(image::ImageFormat::Jpeg),
        "image/webp" => Ok(image::ImageFormat::WebP),
        _ => Err(CaptureInboxError::InvalidImage),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        MAX_ENCRYPTED_CAPTURE_BYTES, image_format_for_media_type, read_encrypted_blob,
        remove_encrypted_blob, stage_encrypted_capture_asset,
    };
    use crate::modules::capture_inbox::CaptureInboxError;

    #[test]
    fn rejects_paths_outside_the_blob_repository() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let error = read_encrypted_blob(directory.path(), "../library.db")
            .expect_err("parent traversal must fail");
        assert!(matches!(error, CaptureInboxError::InvalidAssetPath));
    }

    #[test]
    fn bounded_reads_reject_oversized_ciphertext() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("oversized.enc");
        let file = fs::File::create(&path).expect("create sparse file");
        file.set_len(MAX_ENCRYPTED_CAPTURE_BYTES + 1)
            .expect("set sparse length");
        let error = read_encrypted_blob(directory.path(), "oversized.enc")
            .expect_err("oversized ciphertext must fail");
        assert!(matches!(error, CaptureInboxError::InvalidImage));
    }

    #[test]
    fn missing_blob_removal_is_idempotent() {
        let directory = tempfile::tempdir().expect("temporary directory");
        remove_encrypted_blob(directory.path(), "missing.enc").expect("missing is already removed");
    }

    #[test]
    fn accepts_only_supported_capture_image_formats() {
        assert_eq!(
            image_format_for_media_type("image/png").expect("png"),
            image::ImageFormat::Png
        );
        assert_eq!(
            image_format_for_media_type("image/jpeg").expect("jpeg"),
            image::ImageFormat::Jpeg
        );
        assert_eq!(
            image_format_for_media_type("image/webp").expect("webp"),
            image::ImageFormat::WebP
        );
        assert!(image_format_for_media_type("image/svg+xml").is_err());
    }

    #[test]
    fn rollback_preserves_a_preexisting_unowned_capture_blob() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let asset_id = "0198a000-0000-7000-8000-000000000001";
        let final_path = directory
            .path()
            .join("blobs")
            .join(&asset_id[..2])
            .join(format!("{asset_id}.mtb"));
        let mut staged =
            stage_encrypted_capture_asset(directory.path(), asset_id.to_owned(), b"new ciphertext")
                .expect("stage asset");
        fs::create_dir_all(final_path.parent().expect("final parent")).expect("create parent");
        fs::write(&final_path, b"existing ciphertext").expect("write sentinel");

        let error = staged
            .promote()
            .expect_err("pre-existing target must be rejected");
        assert!(
            matches!(error, CaptureInboxError::Io(ref io) if io.kind() == std::io::ErrorKind::AlreadyExists)
        );
        drop(staged);

        assert_eq!(
            fs::read(final_path).expect("sentinel survives rollback"),
            b"existing ciphertext"
        );
    }

    #[test]
    fn rollback_removes_a_capture_blob_promoted_by_this_owner() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let asset_id = "0198a000-0000-7000-8000-000000000002";
        let staged_path = directory
            .path()
            .join(".staging")
            .join(format!("{asset_id}.capture.tmp"));
        let final_path = directory
            .path()
            .join("blobs")
            .join(&asset_id[..2])
            .join(format!("{asset_id}.mtb"));
        let mut staged = stage_encrypted_capture_asset(
            directory.path(),
            asset_id.to_owned(),
            b"owned ciphertext",
        )
        .expect("stage asset");

        staged.promote().expect("promote asset");
        assert!(final_path.exists());
        drop(staged);

        assert!(!staged_path.exists());
        assert!(!final_path.exists());
    }

    #[test]
    fn commit_keeps_the_promoted_capture_blob() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let asset_id = "0198a000-0000-7000-8000-000000000003";
        let final_path = directory
            .path()
            .join("blobs")
            .join(&asset_id[..2])
            .join(format!("{asset_id}.mtb"));
        let mut staged = stage_encrypted_capture_asset(
            directory.path(),
            asset_id.to_owned(),
            b"committed ciphertext",
        )
        .expect("stage asset");

        staged.promote().expect("promote asset");
        staged.mark_committed();
        drop(staged);

        assert_eq!(
            fs::read(final_path).expect("committed asset survives owner drop"),
            b"committed ciphertext"
        );
    }

    #[test]
    fn dropping_before_promotion_removes_the_staged_capture_blob() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let asset_id = "0198a000-0000-7000-8000-000000000004";
        let staged_path = directory
            .path()
            .join(".staging")
            .join(format!("{asset_id}.capture.tmp"));
        let staged = stage_encrypted_capture_asset(
            directory.path(),
            asset_id.to_owned(),
            b"temporary ciphertext",
        )
        .expect("stage asset");

        assert!(staged_path.exists());
        drop(staged);

        assert!(!staged_path.exists());
    }
}
