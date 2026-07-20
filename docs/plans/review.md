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
- A selected 1–100 problem deck can start a restart-safe simulated exam. The learner first
  moves through question-only cards, then explicitly enters a grading pass where answers
  become available and each right/wrong decision updates FSRS and the persisted result.
- The current review problem is derived by Rust from the active session rather than accepted
  as a page-provided ID. During the exam answering pass, answer assets never enter the Vue DTO.
- Each profile can choose no focus interlude, one Schulte warm-up at session start, or a
  Schulte break after every ten completed cards. The policy is frozen per ordinary session;
  exams always opt out. Board progress survives restart, correct choices persist before
  fading, stale clients resynchronize, and every round can be skipped.
- The report links to a lazy, date-grouped review-history workspace. It supports bounded
  range/rating/subject/note filters, opaque cursor pagination, archived-problem history, and
  a desktop panel/mobile sheet that audits immutable event versions separately from the
  current schedule projection. Runtime identity scopes every Rust query, raw device IDs never
  cross into Vue, and event IDs never enter the route.

Still remaining from the complete review plan:

- Complete two-device deterministic schedule tests after the sync transport is connected.

Exit: two devices derive the same due date from the same event set.
