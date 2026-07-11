# Export plan

1. Persist selection and layout as immutable, syncable ExportSnapshot data.
2. Generate original-image folders and alternating/grouped DOCX layouts in Rust.
3. Validate generated DOCX files with Microsoft Word fixtures and visual snapshots.
4. Build reports from local read models with lazy-loaded charts.

Exit: exported files reproduce from a snapshot and never need cloud synchronization.
