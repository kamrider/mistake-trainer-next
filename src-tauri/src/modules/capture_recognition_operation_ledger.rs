use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecognitionOperationLedger {
    pub(super) source_items: Vec<RecognitionLedgerSource>,
    pub(super) created_items: Vec<RecognitionLedgerItem>,
    pub(super) created_drafts: Vec<RecognitionLedgerDraft>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecognitionLedgerSource {
    pub(super) item_id: String,
    pub(super) asset_id: String,
    pub(super) superseded_by_derivation_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecognitionLedgerItem {
    pub(super) item_id: String,
    pub(super) asset_id: String,
    pub(super) derivation_id: String,
    pub(super) source_sequence: i64,
    pub(super) staged_role: String,
    pub(super) draft_id: Option<String>,
    pub(super) role: Option<String>,
    pub(super) position: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecognitionLedgerDraft {
    pub(super) draft_id: String,
    pub(super) position: i64,
}
