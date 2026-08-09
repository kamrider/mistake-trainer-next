use serde_json::Value;

use crate::{
    application::ports::sync::RemotePullChange,
    modules::sync_store::{
        WireAsset, WireExportSnapshot, WireProblemAggregate, WireProfile, WireReviewEvent,
        WireTombstone,
    },
};

use super::{MAX_ASSET_BYTES, PAGE_SIZE, SyncPullError, validate_uuid};

#[derive(Clone, Debug)]
pub(super) enum DecodedChange {
    Profile(WireProfile),
    Asset(WireAsset),
    Problem(WireProblemAggregate),
    Review(WireReviewEvent),
    Export(WireExportSnapshot),
    Tombstone(WireTombstone),
}

pub(super) fn validate_page(
    page: &[RemotePullChange],
    after: i64,
    remote_user_id: &str,
) -> Result<(), SyncPullError> {
    if page.len() > PAGE_SIZE {
        return Err(SyncPullError::InvalidChange);
    }
    let mut previous = after;
    for change in page {
        if change.change_seq <= previous
            || change.change_seq < 1
            || change.entity_id.is_empty()
            || change.entity_id.len() > 80
            || !matches!(change.operation.as_str(), "upsert" | "append" | "delete")
        {
            return Err(SyncPullError::InvalidChange);
        }
        previous = change.change_seq;
        let object = change
            .payload
            .as_object()
            .ok_or(SyncPullError::InvalidChange)?;
        let account = object
            .get("accountId")
            .or_else(|| object.get("account_id"))
            .and_then(Value::as_str)
            .ok_or(SyncPullError::InvalidChange)?;
        if account != remote_user_id {
            return Err(SyncPullError::InvalidChange);
        }
    }
    Ok(())
}

pub(super) fn decode_page(
    page: &[RemotePullChange],
    remote_user_id: &str,
) -> Result<Vec<DecodedChange>, SyncPullError> {
    let mut decoded = Vec::with_capacity(page.len());
    for change in page {
        let payload = without_account(&change.payload, remote_user_id)?;
        let value = match change.entity_type.as_str() {
            "learner_profile" if change.operation == "upsert" => {
                DecodedChange::Profile(from_value(payload)?)
            }
            "asset" if change.operation == "upsert" => {
                let asset: WireAsset = from_value(payload)?;
                validate_remote_asset(&asset, remote_user_id)?;
                DecodedChange::Asset(asset)
            }
            "problem" if change.operation == "upsert" => {
                DecodedChange::Problem(from_value(payload)?)
            }
            "review_event" if matches!(change.operation.as_str(), "upsert" | "append") => {
                DecodedChange::Review(from_value(payload)?)
            }
            "export_snapshot" if change.operation == "upsert" => {
                DecodedChange::Export(from_value(payload)?)
            }
            "problem" | "learner_profile" | "asset" | "review_event" | "export_snapshot"
                if change.operation == "delete" =>
            {
                DecodedChange::Tombstone(from_value(payload)?)
            }
            _ => return Err(SyncPullError::InvalidChange),
        };
        decoded.push(value);
    }
    Ok(decoded)
}

fn without_account(value: &Value, remote_user_id: &str) -> Result<Value, SyncPullError> {
    let object = value.as_object().ok_or(SyncPullError::InvalidChange)?;
    let account = object
        .get("accountId")
        .or_else(|| object.get("account_id"))
        .and_then(Value::as_str)
        .ok_or(SyncPullError::InvalidChange)?;
    if account != remote_user_id {
        return Err(SyncPullError::InvalidChange);
    }
    let mut clean = object.clone();
    clean.remove("accountId");
    clean.remove("account_id");
    Ok(Value::Object(clean))
}

fn from_value<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, SyncPullError> {
    serde_json::from_value(value).map_err(|_| SyncPullError::InvalidChange)
}

fn validate_remote_asset(asset: &WireAsset, remote_user_id: &str) -> Result<(), SyncPullError> {
    validate_uuid(&asset.id)?;
    if asset.plaintext_sha256.len() != 64
        || !asset
            .plaintext_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || asset.byte_length <= 0
        || usize::try_from(asset.byte_length)
            .ok()
            .is_none_or(|length| length > MAX_ASSET_BYTES)
        || !matches!(
            asset.media_type.as_str(),
            "image/jpeg" | "image/png" | "image/webp"
        )
        || asset.storage_object != format!("{remote_user_id}/{}", asset.plaintext_sha256)
    {
        return Err(SyncPullError::InvalidAsset);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const REMOTE_USER_ID: &str = "33333333-3333-4333-8333-333333333333";

    fn change(change_seq: i64, account_id: &str) -> RemotePullChange {
        RemotePullChange {
            change_seq,
            entity_type: "learner_profile".to_owned(),
            entity_id: format!("profile-{change_seq}"),
            operation: "upsert".to_owned(),
            payload: json!({ "accountId": account_id }),
        }
    }

    #[test]
    fn accepts_a_strictly_ordered_page_owned_by_the_remote_account() {
        let page = vec![change(4, REMOTE_USER_ID), change(5, REMOTE_USER_ID)];

        assert!(validate_page(&page, 3, REMOTE_USER_ID).is_ok());
    }

    #[test]
    fn rejects_non_increasing_sequences_and_foreign_accounts() {
        let repeated = vec![change(4, REMOTE_USER_ID), change(4, REMOTE_USER_ID)];
        let foreign = vec![change(4, "44444444-4444-4444-8444-444444444444")];

        assert!(matches!(
            validate_page(&repeated, 3, REMOTE_USER_ID),
            Err(SyncPullError::InvalidChange)
        ));
        assert!(matches!(
            validate_page(&foreign, 3, REMOTE_USER_ID),
            Err(SyncPullError::InvalidChange)
        ));
    }

    #[test]
    fn rejects_pages_larger_than_the_transport_limit() {
        let page = (1..=PAGE_SIZE + 1)
            .map(|sequence| change(sequence as i64, REMOTE_USER_ID))
            .collect::<Vec<_>>();

        assert!(matches!(
            validate_page(&page, 0, REMOTE_USER_ID),
            Err(SyncPullError::InvalidChange)
        ));
    }
}
