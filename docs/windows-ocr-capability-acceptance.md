# Windows Optional OCR Capability Acceptance

This checklist verifies the optional setup boundary only. It does not certify OCR
accuracy and does not authorize automatic question cropping.

## Safety invariants

- [ ] Start the application offline and open every non-Settings route. Confirm there is
  no OCR network request, startup delay, model prompt, or behavior change.
- [ ] Open Settings. Confirm the page shows logical processor count, physical memory,
  component-volume headroom, and AVX2 support without showing a filesystem path.
- [ ] Confirm opening or revisiting Settings does not download anything.
- [ ] Confirm the encrypted library remains usable when capability status fails.
- [ ] Confirm capture, library, review, export, sync, backup, restore, and storage
  migration behave identically with no model installed.
- [ ] Confirm model files are outside `library`, absent from encrypted backup manifests,
  absent from sync outbox/object storage, and absent from storage-migration copies.

## Hardware tiers

- [ ] On a machine below 2 logical processors, 4 GiB memory, or 1 GiB free component
  space, confirm the result is `保持手工整理模式` and no model download is offered.
- [ ] On a 2-core/4-GiB-class machine, confirm the result recommends the existing manual
  flow and no model download is offered.
- [ ] On a 4-core/8-GiB AVX2 machine with at least 2 GiB free space, confirm only
  PP-OCRv6 small is recommended and medium is hidden.
- [ ] On an 8-core/16-GiB AVX2 machine with at least 4 GiB free space, confirm small is
  recommended and medium is also available as an optional tier.
- [ ] On x64 hardware without AVX2, confirm the model action is withheld and existing
  workflows stay available.

## Download and integrity

- [ ] Select the small model. Confirm no request occurs until the confirmation checkbox
  is selected and `确认并下载` is pressed.
- [ ] Confirm the dialog states the exact rounded download size, ModelScope/RapidAI
  source, Apache-2.0 lineage, local-only storage, SHA-256 verification, and that
  installation does not enable recognition.
- [ ] Disconnect the network during each model file. Confirm a fixed retryable message,
  no installed state, and cleanup of the private staging directory on the next status
  check.
- [ ] Return HTTP error, truncated bytes, excess bytes, and a same-size wrong payload
  from a test proxy. Confirm all are rejected and any previously verified bundle remains.
- [ ] Complete the small download and restart. Confirm both ONNX files pass fixed length
  and SHA-256 checks and the UI reports `已校验`.
- [ ] Start two installation/removal/status mutations concurrently through command
  invocation. Confirm the process-wide capability mutex serializes them.
- [ ] Remove the model, restart, and confirm only the optional component directory was
  removed; the encrypted library and every question remain unchanged.
- [ ] Confirm OpenCV says `产品运行时尚未发布` and has no action button.

## Accessibility and responsiveness

- [ ] Navigate the Settings directory to `智能识别` using keyboard only.
- [ ] Complete and cancel both confirmation dialogs using keyboard only.
- [ ] With reduced motion enabled, confirm the panel has no required animation.
- [ ] During a download, navigate to capture, library, and review. Confirm UI interaction
  remains responsive and no database/profile lock is held.
- [ ] Confirm all failure and success outcomes are announced as alert/status text and do
  not rely only on color.

## Automated regression

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm bindings:check
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
```

All commands must pass before release. The production OCR worker, question-region
suggestions, content extraction, UVDoc, PP-DocLayout-M, and formula recognition remain
disabled until their separate evidence and acceptance gates pass.
