# Capture workbench 150-image performance lab

This local-only lab mounts the real `CaptureWorkspace` with 150 synthetic image items. It does
not open the product database, create capture assets, or count as recognition-accuracy evidence.

Start it from the repository root:

```powershell
.\scripts\capture-workbench-perf.ps1
```

Before recording evidence, verify that the isolated lab still compiles:

```powershell
.\node_modules\.bin\vue-tsc.cmd --noEmit -p labs/capture-workbench-perf/tsconfig.json
.\node_modules\.bin\vite.cmd build labs/capture-workbench-perf
```

Open the printed `127.0.0.1` URL, choose **运行往返滚动**, then drag a visible thumbnail onto
**自动生成一道新题**. Read `window.__capturePerfEvidence` for the auditable values:

- `sampleCount` must be 150;
- `previewCachePeak` and `previewCacheSize` must never exceed 40;
- `longTaskCount` must be zero for the measured scroll;
- `maxLongTaskMs` must remain below 50 ms;
- `completedDragCount` must become at least one.

Run the final acceptance on the 4-core/8 GB Windows reference PC. Results from a faster
development machine are useful regression evidence but do not replace that reference-machine
gate.
