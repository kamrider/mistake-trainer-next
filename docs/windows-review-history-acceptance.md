# Windows review-history acceptance

## Entry and identity

- Open **学习报告** and activate **查看完整复习历史**.
- Confirm `/report/history` opens lazily while the sidebar still marks **学习报告** current.
- Confirm no event ID appears in the URL after opening or closing a detail.
- Switch learner profiles and confirm the history is replaced rather than mixed or retained.

## List, filters, and pagination

- Verify genuine empty history, an initial loading state, and a large history containing more
  than 20 events.
- Exercise `近 7 天`, `近 30 天`, and `全部时间`; all four ratings; every available subject;
  ordinary note search; and literal `%`, `_`, and `\` searches.
- Verify archived and trashed problem events remain visible with their current status label.
- Create equal-time events and use **加载更多**. No row may repeat or disappear at the page
  boundary.
- Simulate a replacement read failure and an append failure. Existing successful rows remain
  visible, are labelled stale, and can be retried.

## Audit detail

- Open current-version and legacy-version events. Verify algorithm and parameter badges are
  independent and reflect immutable event values.
- Verify **本机设备** and **其他设备** without displaying the underlying device ID.
- Verify ordinal/total counts, full note, rating, occurrence time, duration, and problem status.
- Verify current due time, stability, difficulty, and projection algorithm are labelled as the
  current projection, never as the event-time schedule.
- Rapidly select two rows while delaying the first detail response. Only the latest detail may
  appear.

## Windows layout and accessibility

- At 1280×900, verify the timeline and sticky detail panel remain readable and independently
  scrollable.
- At 760×900 and 390×844, verify detail becomes a bottom sheet, the close target is at least
  44×44 px, `scrollWidth === clientWidth`, and no seal or action is clipped.
- Close with the button and Escape. Focus returns to the row that opened the sheet.
- Complete all filter, row, pagination, retry, and close operations using only the keyboard.
- Enable Windows high contrast and ensure selected rows, focus, controls, and badges remain
  distinguishable.
- Enable **减少动态效果** and confirm data appears immediately without row, sheet, or loading
  motion blocking interaction.

## Offline and diagnostics

- Disconnect the network, restart the app, and confirm local history and detail still load.
- Confirm a failed command shows only the stable Chinese message and diagnostic ID. SQL,
  database paths, account/profile IDs, event IDs, and raw device IDs must not be visible.
- Confirm the browser console has no warning/error and the Windows app can close and reopen
  without losing existing review events.
