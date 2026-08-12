# PDF And Full Exam Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Import a local PDF or full exam as ordered page images into the existing encrypted capture batch, with page progress, cancellation, capacity limits, and no network or plaintext disk spill.

**Architecture:** Use bundled `pdfjs-dist` in a browser worker to validate and render one page at a time to PNG in memory. Convert rendered blobs to `File` objects named `<document>-pNNN.png` and feed them through the existing two-worker `useCaptureFileImport` path so encryption, deduplication, ordering, and batch recovery remain canonical.

**Tech Stack:** Vue 3, TypeScript, `pdfjs-dist`, Canvas, existing Tauri image import command, Vitest.

## Global Constraints

- PDF processing is offline; production code must make no PDF-related network request.
- Do not write the original PDF or rendered plaintext pages to disk.
- Maximum PDF size: 100 MiB; maximum pages imported into one batch remains 150 including existing images.
- Password-protected, malformed, empty, or oversized PDFs fail without importing partial pages.
- A canceled render stops before the next page and keeps already encrypted pages visible with truthful copy.

---

### Task 1: Add an isolated PDF page renderer

**Files:**
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Create: `src/modules/capture/services/pdfPageRenderer.ts`
- Create: `src/modules/capture/services/pdfPageRenderer.test.ts`

**Interfaces:**
- Consumes: `File` with MIME `application/pdf` or `.pdf` extension.
- Produces: `renderPdfPages(file, options) -> AsyncGenerator<RenderedPdfPage>` where `RenderedPdfPage = { pageNumber: number; pageCount: number; file: File }`.

- [ ] **Step 1: Add the pinned renderer dependency**

Run: `pnpm add pdfjs-dist@5.4.624`

Expected: `package.json` and `pnpm-lock.yaml` contain the exact version range selected by pnpm and no postinstall binary download.

- [ ] **Step 2: Write renderer contract tests with a small checked-in fixture**

```ts
const pages = await collect(renderPdfPages(pdfFile, { maxPages: 150, maxBytes: 100 * 1024 * 1024 }))
expect(pages.map(page => page.pageNumber)).toEqual([1, 2, 3])
expect(pages[0]!.file.name).toBe('数学周测-p001.png')
expect(pages[0]!.file.type).toBe('image/png')
```

Cover malformed bytes, password-required documents, zero pages, >150 pages, >100 MiB metadata rejection, and abort signal behavior.

- [ ] **Step 3: Configure the bundled worker**

```ts
import { GlobalWorkerOptions, getDocument } from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'
GlobalWorkerOptions.workerSrc = workerUrl
```

Load with `{ data: new Uint8Array(await file.arrayBuffer()), isEvalSupported: false }`, render with a white background at a scale capped so width and height remain at most 4,096 pixels, and encode `image/png` through `canvas.toBlob`.

- [ ] **Step 4: Run renderer tests and production build**

Run: `pnpm exec vitest run src/modules/capture/services/pdfPageRenderer.test.ts`

Expected: PASS without network access.

Run: `pnpm build`

Expected: the PDF worker is bundled locally and the production build passes.

- [ ] **Step 5: Commit**

```bash
git add package.json pnpm-lock.yaml src/modules/capture/services/pdfPageRenderer.ts src/modules/capture/services/pdfPageRenderer.test.ts
git commit -m "feat: render local PDF pages offline"
```

### Task 2: Integrate PDF documents with capture import

**Files:**
- Modify: `src/modules/capture/composables/useCaptureFileImport.ts`
- Modify: `src/modules/capture/composables/useCaptureFileImport.test.ts`
- Modify: `src/modules/capture/composables/useCaptureImportWorkflow.ts`
- Modify: `src/modules/capture/composables/useCaptureImportWorkflow.test.ts`

**Interfaces:**
- Consumes: mixed image/PDF `File[]` and remaining batch capacity.
- Produces: `CaptureFileImportOutcome` with `documentReports: Array<{ sourceName: string; pageCount: number; importedCount: number; canceled: boolean }>`.

- [ ] **Step 1: Write mixed-order and capacity tests**

```ts
expect(importBytes.mock.calls.map(call => call[0].sourceName)).toEqual([
  'cover.png', '数学周测-p001.png', '数学周测-p002.png', 'answer.png',
])
expect(importBytes.mock.calls.map(call => call[0].sourceSequence)).toEqual([0, 1, 2, 3])
```

Cover a PDF that exceeds remaining capacity, rendering failure before page one, failure after encrypted pages, cancellation, and a mixed drop containing unsupported files.

- [ ] **Step 2: Extend import progress**

```ts
export interface CaptureFileImportProgress {
  completed: number
  total: number
  failed: number
  sourceName?: string
  phase?: 'reading_pdf' | 'rendering_pdf' | 'encrypting_images'
}
```

Expand PDFs sequentially to preserve document order, then pass page files to the existing bounded image workers. Capacity applies to rendered pages plus ordinary images, not source-file count.

- [ ] **Step 3: Replace unsupported-PDF copy**

The notice lists successful document page counts, skipped pages due to the 150-item limit, cancellation, malformed/password failures, and remaining unsupported formats. It must never claim that all PDF pages imported when only part did.

- [ ] **Step 4: Run import workflow tests**

Run: `pnpm exec vitest run src/modules/capture/composables/useCaptureFileImport.test.ts src/modules/capture/composables/useCaptureImportWorkflow.test.ts`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/modules/capture/composables/useCaptureFileImport.ts src/modules/capture/composables/useCaptureFileImport.test.ts src/modules/capture/composables/useCaptureImportWorkflow.ts src/modules/capture/composables/useCaptureImportWorkflow.test.ts
git commit -m "feat: import ordered PDF exam pages"
```

### Task 3: Expose full-exam import and cancellation in the capture UI

**Files:**
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`
- Create: `docs/windows-pdf-import-acceptance.md`

**Interfaces:**
- Consumes: PDF-aware file importer and progress phases.
- Produces: file picker accepting `.pdf,image/png,image/jpeg,image/webp`, progress with page/document name, cancel button, and recovery copy.

- [ ] **Step 1: Add UI behavior tests**

```ts
expect(screen.getByLabelText('导入图片或 PDF 试卷')).toHaveAttribute(
  'accept', '.pdf,application/pdf,image/png,image/jpeg,image/webp',
)
await user.click(screen.getByRole('button', { name: '取消 PDF 导入' }))
expect(cancelImport).toHaveBeenCalledTimes(1)
```

- [ ] **Step 2: Implement accessible progress**

Announce `正在渲染 数学周测.pdf，第 8 / 20 页` through `role="status"`. Disable only conflicting import actions; manual organization of already encrypted pages remains available after cancellation or failure.

- [ ] **Step 3: Run frontend checks**

Run: `pnpm exec vitest run src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/CaptureView.test.ts`

Expected: PASS.

Run: `pnpm lint`

Expected: zero warnings.

Run: `pnpm typecheck`

Expected: exit code 0.

- [ ] **Step 4: Run Windows acceptance**

Test a 1-page PDF, a 150-page exam, mixed portrait/landscape pages, Chinese filename, drag/drop, picker import, password protection, corrupt PDF, cancellation, restart after partial encrypted import, and offline mode. Record the PDF SHA-256 and build version without storing document contents.

- [ ] **Step 5: Commit**

```bash
git add src/modules/capture/components/CaptureWorkspace.vue src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/CaptureView.vue src/app/views/CaptureView.test.ts docs/windows-pdf-import-acceptance.md
git commit -m "feat: add full-exam PDF import workflow"
```
