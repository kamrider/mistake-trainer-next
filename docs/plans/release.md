# Release plan

1. Enforce lint, typecheck, unit, integration, database, E2E and build gates.
2. Add Windows signing, signed updater, installer metadata and rollback notes.
3. Verify cold start, JS budgets, 60 Hz review interactions and reduced motion.
4. Produce diagnostics that contain correlation IDs but no secrets or image content.

## Implemented locally

- Settings can export a versioned JSON diagnostic report through a native folder picker.
- The Rust-owned schema contains only application/platform versions, fixed integrity status,
  and bounded aggregate counts for the library and sync queue.
- The report and its typed receipt exclude pictures, question/answer content, subjects, tags,
  notes, identities, credentials, cloud endpoints, original file names, and local paths.
- A real encrypted-database test fills every sensitive field with unique sentinels and proves
  none occur in the serialized report.
- Reports are flushed to a same-directory temporary file and atomically renamed; failures
  remove the temporary file and never overwrite an existing report.
- Native cancellation is neutral, repeated clicks are guarded, and the success receipt exposes
  only a report correlation ID, fixed file label, generation time, and warning count.

## Remaining release acceptance

- Configure Windows code signing, signed updates, installer metadata, and rollback notes.
- Measure cold start and 60 Hz review/capture interactions on the Windows reference machine.
- Run clean-machine installation, upgrade, uninstall, and rollback smoke tests.

Exit: a signed Windows v1 installer passes clean-machine smoke tests.
