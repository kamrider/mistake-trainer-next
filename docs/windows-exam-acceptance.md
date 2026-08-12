# Windows Exam Mode Acceptance

## Real-file release evidence

- [x] Automated PDF import tests cover multipage order, cancel during parsing, partial success,
  malformed input, password errors and bounded page/byte limits.
- [x] The import-to-Rust boundary keeps each rendered page as a typed byte buffer and avoids an
  additional spread-copy proportional to page size.
- [ ] On each supported Windows architecture, import the approved non-private real-file corpus:
  multipage scanned, password-protected, malformed, empty and near-limit PDFs.
- [ ] Open a representative generated DOCX in supported desktop Word and inspect pagination,
  heading styles, missing-image copy and image readability.

Use a non-production learner profile with at least three active problems. At least one problem
must contain multiple question images and multiple answer images.

## Start and answer secrecy

1. Open **题库**, select three cards in a non-list order, and choose **模拟考试 3 道题**.
2. Confirm the training route contains no problem IDs or query parameters.
3. Confirm the first card shows question media, **上一题** is disabled, and no answer media,
   **显示答案**, or rating control exists.
4. Use Right Arrow and the visible next button. Confirm the chosen order is preserved, the
   card moves in the correct direction, and full-size question inspection remains available.
5. Close the app on the second question, reopen it, and enter **训练室**. Confirm the same
   question and the “已恢复上次进度” indicator appear.

## Grading and recovery

1. Move to the final question and choose **开始核对答案**. Confirm focus moves to the grading
   heading and the first problem returns with its answer media visible.
2. Mark one problem **答错** and one **答对**. Confirm each successful decision advances once;
   a failed persistence attempt must leave the same card and enable retry.
3. Close and reopen during grading. Confirm the next ungraded problem appears and the already
   persisted correct/wrong counters are retained.
4. Finish the exam. Confirm the result shows the exact correct count, wrong count, and rounded
   accuracy. Confirm the report/review history includes the new review events and the local
   outbox count increases.

## Keyboard, motion, and layout

1. During answering, use Left/Right Arrow; on the final card use Enter to begin grading.
2. During grading, use `1` for wrong and `2` for right. Escape leaves the room without deleting
   unfinished progress.
3. At 1280×720 and 760×900, confirm no horizontal page overflow, question images remain
   readable, and the phase label clearly distinguishes “独立作答” from “核对答案”.
4. Enable Windows **Animation effects: Off** or browser `prefers-reduced-motion`. Confirm card,
   answer, and completion transitions disappear without removing any state or control.

## Data invariant inspection

For a debug fixture, verify an answering-pass call to `review_current_problem` serializes only
`role=question` assets. After grading begins, the same command may serialize both roles. A direct
`review_submit` during answering must create zero review events, zero schedule changes, and zero
outbox operations.
