# Review plan

1. Create append-only ReviewEvent and deterministic ScheduleState projections.
2. Integrate FSRS at 0.90 desired retention and persist algorithm/parameter versions.
3. Build due, manual, exam, timer and focus sessions with crash-safe resume.
4. Support simple Again/Good and opt-in four-rating controls.

Implemented vertical slices:

- Due sessions persist progress and resume after navigation or restart.
- The library can build an ordered 1–100 problem manual deck. Rust validates and stores
  the deck before the page changes, and the review route contains no problem IDs.
- Both due and manual modes use the same transactional rating path, timer, keyboard
  controls, ordered media lightbox, and simple/advanced scoring UI.

Exit: two devices derive the same due date from the same event set.
