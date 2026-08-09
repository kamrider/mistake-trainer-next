# Capture Crop Return Focus

## Goal

Return keyboard focus to the crop launcher after cancel or successful save, including when the batch refresh replaces the original launcher node. Keep focus in the editor after a failed save and restore the launcher after an open failure.

## Tasks

- [x] Add CaptureView integration tests for cancel, successful save, preview failure, and save failure.
- [x] Capture the crop launcher at the page lifecycle boundary.
- [x] Add stable crop-item identifiers to every launcher variant.
- [x] Restore focus only when the editor actually closes; clear stale targets when leaving the detail view.
- [x] Run focused tests, lint, typecheck, build, and the full frontend suite.
