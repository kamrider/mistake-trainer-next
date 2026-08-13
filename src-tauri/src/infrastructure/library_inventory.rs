use std::path::Path;

use crate::application::library_inventory::LibraryArtifactState;

pub fn inspect_library_artifacts(
    library_root: &Path,
) -> Result<LibraryArtifactState, std::io::Error> {
    for path in [library_root.join("library.db"), library_root.join("assets")] {
        match std::fs::symlink_metadata(path) {
            Ok(_) => return Ok(LibraryArtifactState::Present),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(LibraryArtifactState::Absent)
}
