# Capture Crop Busy and Focus Boundary

## Goal

Keep the crop recipe immutable while an apply/save transaction is running, and preserve keyboard focus when a region row is deleted.

## Tasks

- [x] Add component tests for the busy interaction boundary and region deletion focus continuity.
- [x] Guard every recipe mutation path at the function boundary while `busy` is true.
- [x] Disable region mutation controls and expose the dialog busy state semantically.
- [x] Move focus to the selected adjacent region after deletion and announce region-count changes.
- [x] Run focused tests, lint, typecheck, build, and the full frontend suite.
