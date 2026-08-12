# PDF Import Memory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the JavaScript array expansion that multiplies PDF-page memory while preserving incremental rendering, cancellation, encrypted storage, and durable ordering.

**Architecture:** Keep at most one rendered page in flight and pass its `Uint8Array` directly through Tauri's supported typed-array serializer. Introduce a narrow frontend transport type rather than editing generated Specta bindings; Rust continues to deserialize `Vec<u8>` and allocate source order inside the serialized transaction.

**Tech Stack:** TypeScript 5.9, Tauri API 2.11, Vue composables, Vitest.

## Global Constraints

- Never write rendered plaintext PDF pages to disk.
- Keep PDF parsing and page rendering cancelable.
- Keep one page/import in flight so memory stays bounded independently of document page count.
- Preserve `sourceSequence: null` server allocation.

---

### Task 1: Typed-array transport

**Files:**
- Modify: `src/modules/capture/composables/useCaptureFileImport.ts`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/modules/capture/composables/useCaptureFileImport.test.ts`

**Interfaces:**
- Produces: `CaptureImportBinaryInput = Omit<CaptureImportBytesInput, 'bytes'> & { bytes: Uint8Array }`.

- [x] **Step 1: Write a failing transport-contract test**

```ts
expect(importBytes.mock.calls[0]![0].bytes).toBeInstanceOf(Uint8Array)
expect(importBytes.mock.calls[0]![0].bytes).toEqual(new Uint8Array([1]))
```

- [x] **Step 2: Run the focused test and confirm current `number[]` behavior**

Run: `pnpm test -- src/modules/capture/composables/useCaptureFileImport.test.ts`
Expected: FAIL because `[...new Uint8Array(buffer)]` produces an Array.

- [x] **Step 3: Pass the typed array without expansion**

```ts
const bytes = new Uint8Array(await file.arrayBuffer())
await options.importBytes({ batchId, clientUploadId, sourceName, sourceSequence: null, bytes })
```

- [x] **Step 4: Adapt only the generated-command boundary**

```ts
importBytes: async input => normalizeAppResult(
  await commands.captureImportBytes(input as CaptureImportBytesInput),
),
```

- [x] **Step 5: Verify importer, types, and production build**

Run: `pnpm test -- src/modules/capture/composables/useCaptureFileImport.test.ts src/modules/capture/services/pdfPageRenderer.test.ts`
Expected: PASS.

Run: `pnpm typecheck && pnpm build`
Expected: PASS.
