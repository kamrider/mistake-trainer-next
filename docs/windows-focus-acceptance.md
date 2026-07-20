# Windows Schulte Focus Acceptance

Use a development library with at least 21 active problems. Run every case in the real
Tauri application; the Vite preview at `/#/review?preview=focus` is only for visual checks.

## Policy and session behavior

- Set the policy to **关闭专注插曲**, start a due session and a manual session, and verify
  both open the first card directly.
- Set **每轮开始前 · 推荐**, start a new due session, and verify one 1–25 board appears
  before the first problem. Exit and reopen halfway through; the board order, next number,
  and elapsed time must resume.
- Change the setting while that session is active. Reopen it and verify its frozen policy
  does not change. Finish or discard it, then verify the next ordinary session uses the new
  preference.
- Set **每完成 10 题**, complete cards 1–9 without interruption, then verify card 10 is
  durably rated before a board appears. Skip or finish the board and continue at card 11.
  Repeat at card 20. If card 10 or 20 is the final card, no board may appear afterward.
- Start a simulated exam under both non-off preferences. Verify neither answering nor
  grading ever inserts a focus board.

## Persistence and error behavior

- Click a number other than the announced next number. Verify only a short cinnabar shake
  and “请先找到 N” appear; restart and confirm persisted progress did not change.
- Click the correct number, then immediately close and reopen the app. Verify that tile is
  already completed and the next expected number is authoritative.
- Complete number 25. Verify “这一轮已保存” appears briefly before the next problem loads.
- Choose **跳过，继续训练**, restart, and verify the skipped board does not return.
- Simulate a response lost after a successful selection. The page must reload the Rust
  state, show “已恢复到最新位置”, and allow continuing without a manual refresh.
- While a board is active, verify stale rating/current-problem calls are rejected and do
  not append a review event or reveal answer media.

## Keyboard and accessibility

- Tab into the board. Exactly one unfinished tile is in the tab order.
- Arrow keys move spatially, Home moves to the first unfinished tile, End to the last, and
  Enter activates the focused tile. After a correct activation, focus moves to a still
  enabled tile instead of remaining on the disabled one.
- Verify **退出训练台** and **跳过，继续训练** are keyboard reachable, and both targets
  are at least 44 px high.
- With Windows **Animation effects** disabled or `prefers-reduced-motion: reduce`, verify
  wrong feedback, completed tiles, and the final seal have no delayed animation dependency.
- With Windows high contrast enabled, verify numbers, focus outlines, status text, and both
  actions remain visible.

## Layout and motion

- At 1280×900 and 760×900, verify all 25 numbers are readable and no tile is clipped.
- At a 390 px mobile-width desktop viewport, assert `scrollWidth === clientWidth`; the grid
  may extend vertically but must never create horizontal scrolling.
- The announced next number may appear in the status text, but its tile location must not
  receive a unique color, elevation, or other visual hint.
- Normal motion uses only transform and opacity. Correct tiles disappear only after the
  command succeeds; a failed command leaves the board visually unchanged and retryable.

## Release evidence

Record the app version, Windows build, policy exercised, viewport, reduced-motion/high-
contrast state, and pass/fail result. Attach screenshots only after confirming they contain
no private question images or account identifiers.
