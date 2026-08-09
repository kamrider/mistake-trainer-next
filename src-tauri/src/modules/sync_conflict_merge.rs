use serde_json::Value;

use crate::modules::sync_store::{WireExportSnapshot, WireProblemAggregate, WireProfile};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FieldConflict {
    pub field_name: &'static str,
    pub local_value: Value,
    pub remote_value: Value,
    pub base_revision: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MergeAction<T> {
    ApplyRemote(T),
    ApplyMergedAndEnqueue(T),
    ApplyPartialWithConflicts {
        value: T,
        conflicts: Vec<FieldConflict>,
    },
}

enum FieldDecision {
    Value(Value),
    Conflict { local: Value, remote: Value },
}

fn merge_field(base: Option<&Value>, local: &Value, remote: &Value) -> FieldDecision {
    let Some(base) = base else {
        return if local == remote {
            FieldDecision::Value(remote.clone())
        } else {
            FieldDecision::Conflict {
                local: local.clone(),
                remote: remote.clone(),
            }
        };
    };
    if local == remote || local == base {
        FieldDecision::Value(remote.clone())
    } else if remote == base {
        FieldDecision::Value(local.clone())
    } else {
        FieldDecision::Conflict {
            local: local.clone(),
            remote: remote.clone(),
        }
    }
}

pub(crate) fn merge_problem_versions(
    local: Option<&WireProblemAggregate>,
    base: Option<&WireProblemAggregate>,
    remote: &WireProblemAggregate,
    now_utc_ms: i64,
) -> Result<MergeAction<WireProblemAggregate>, serde_json::Error> {
    let Some(local) = local else {
        return Ok(MergeAction::ApplyRemote(remote.clone()));
    };
    let base_revision = base.map_or(local.revision.min(remote.revision), |value| value.revision);
    let fields = [
        (
            "subject",
            base.map(|value| serde_json::to_value(&value.subject))
                .transpose()?,
            serde_json::to_value(&local.subject)?,
            serde_json::to_value(&remote.subject)?,
        ),
        (
            "tags",
            base.map(|value| serde_json::to_value(&value.tags))
                .transpose()?,
            serde_json::to_value(&local.tags)?,
            serde_json::to_value(&remote.tags)?,
        ),
        (
            "note",
            base.map(|value| serde_json::to_value(&value.note))
                .transpose()?,
            serde_json::to_value(&local.note)?,
            serde_json::to_value(&remote.note)?,
        ),
        (
            "status",
            base.map(|value| serde_json::to_value(&value.status))
                .transpose()?,
            serde_json::to_value(&local.status)?,
            serde_json::to_value(&remote.status)?,
        ),
        (
            "timeLimitSeconds",
            base.map(|value| serde_json::to_value(value.time_limit_seconds))
                .transpose()?,
            serde_json::to_value(local.time_limit_seconds)?,
            serde_json::to_value(remote.time_limit_seconds)?,
        ),
        (
            "assets",
            base.map(|value| serde_json::to_value(&value.assets))
                .transpose()?,
            serde_json::to_value(&local.assets)?,
            serde_json::to_value(&remote.assets)?,
        ),
    ];
    let mut merged = serde_json::to_value(remote)?;
    let merged_object = merged
        .as_object_mut()
        .expect("wire problems always serialize as objects");
    let mut conflicts = Vec::new();
    let mut differs_from_remote = false;
    for (field_name, base_value, local_value, remote_value) in fields {
        match merge_field(base_value.as_ref(), &local_value, &remote_value) {
            FieldDecision::Value(value) => {
                differs_from_remote |= value != remote_value;
                merged_object.insert(field_name.to_owned(), value);
            }
            FieldDecision::Conflict { local, remote } => {
                differs_from_remote = true;
                merged_object.insert(field_name.to_owned(), local.clone());
                conflicts.push(FieldConflict {
                    field_name,
                    local_value: local,
                    remote_value: remote,
                    base_revision,
                });
            }
        }
    }
    let mut merged: WireProblemAggregate = serde_json::from_value(merged)?;
    if !conflicts.is_empty() {
        merged.revision = local.revision.max(remote.revision).saturating_add(1);
        merged.updated_at_utc_ms = now_utc_ms;
        return Ok(MergeAction::ApplyPartialWithConflicts {
            value: merged,
            conflicts,
        });
    }
    if differs_from_remote {
        merged.revision = local.revision.max(remote.revision).saturating_add(1);
        merged.updated_at_utc_ms = now_utc_ms;
        Ok(MergeAction::ApplyMergedAndEnqueue(merged))
    } else {
        Ok(MergeAction::ApplyRemote(remote.clone()))
    }
}

pub(crate) fn merge_profile_versions(
    local: Option<&WireProfile>,
    base: Option<&WireProfile>,
    remote: &WireProfile,
    now_utc_ms: i64,
) -> Result<MergeAction<WireProfile>, serde_json::Error> {
    let Some(local) = local else {
        return Ok(MergeAction::ApplyRemote(remote.clone()));
    };
    let base_name = base
        .map(|value| serde_json::to_value(&value.name))
        .transpose()?;
    match merge_field(
        base_name.as_ref(),
        &serde_json::to_value(&local.name)?,
        &serde_json::to_value(&remote.name)?,
    ) {
        FieldDecision::Value(value) if value == serde_json::to_value(&remote.name)? => {
            Ok(MergeAction::ApplyRemote(remote.clone()))
        }
        FieldDecision::Value(value) => {
            let mut merged = remote.clone();
            merged.name = serde_json::from_value(value)?;
            merged.revision = local.revision.max(remote.revision).saturating_add(1);
            merged.updated_at_utc_ms = now_utc_ms;
            Ok(MergeAction::ApplyMergedAndEnqueue(merged))
        }
        FieldDecision::Conflict {
            local: local_value,
            remote: remote_value,
        } => {
            let mut merged = local.clone();
            merged.revision = local.revision.max(remote.revision).saturating_add(1);
            merged.updated_at_utc_ms = now_utc_ms;
            Ok(MergeAction::ApplyPartialWithConflicts {
                value: merged,
                conflicts: vec![FieldConflict {
                    field_name: "name",
                    local_value,
                    remote_value,
                    base_revision: base
                        .map_or(local.revision.min(remote.revision), |value| value.revision),
                }],
            })
        }
    }
}

pub(crate) fn merge_export_versions(
    local: Option<&WireExportSnapshot>,
    base: Option<&WireExportSnapshot>,
    remote: &WireExportSnapshot,
) -> Result<MergeAction<WireExportSnapshot>, serde_json::Error> {
    let Some(local) = local else {
        return Ok(MergeAction::ApplyRemote(remote.clone()));
    };
    let base_revision = base.map_or(local.revision.min(remote.revision), |value| value.revision);
    let mut merged = remote.clone();
    let mut conflicts = Vec::new();
    let mut differs_from_remote = false;
    let fields = [
        (
            "title",
            base.map(|value| serde_json::to_value(&value.title))
                .transpose()?,
            serde_json::to_value(&local.title)?,
            serde_json::to_value(&remote.title)?,
        ),
        (
            "problemIds",
            base.map(|value| serde_json::to_value(&value.problem_ids))
                .transpose()?,
            serde_json::to_value(&local.problem_ids)?,
            serde_json::to_value(&remote.problem_ids)?,
        ),
        (
            "configuration",
            base.map(|value| value.configuration.clone()),
            local.configuration.clone(),
            remote.configuration.clone(),
        ),
    ];
    for (field_name, base_value, local_value, remote_value) in fields {
        match merge_field(base_value.as_ref(), &local_value, &remote_value) {
            FieldDecision::Value(value) => {
                differs_from_remote |= value != remote_value;
                match field_name {
                    "title" => merged.title = serde_json::from_value(value)?,
                    "problemIds" => merged.problem_ids = serde_json::from_value(value)?,
                    "configuration" => merged.configuration = value,
                    _ => unreachable!(),
                }
            }
            FieldDecision::Conflict { local, remote } => {
                differs_from_remote = true;
                match field_name {
                    "title" => merged.title = serde_json::from_value(local.clone())?,
                    "problemIds" => merged.problem_ids = serde_json::from_value(local.clone())?,
                    "configuration" => merged.configuration = local.clone(),
                    _ => unreachable!(),
                }
                conflicts.push(FieldConflict {
                    field_name,
                    local_value: local,
                    remote_value: remote,
                    base_revision,
                });
            }
        }
    }
    if !conflicts.is_empty() {
        merged.revision = local.revision.max(remote.revision).saturating_add(1);
        return Ok(MergeAction::ApplyPartialWithConflicts {
            value: merged,
            conflicts,
        });
    }
    if differs_from_remote {
        merged.revision = local.revision.max(remote.revision).saturating_add(1);
        Ok(MergeAction::ApplyMergedAndEnqueue(merged))
    } else {
        Ok(MergeAction::ApplyRemote(remote.clone()))
    }
}

pub(crate) fn profile_content(value: &WireProfile) -> Value {
    serde_json::json!({ "name": value.name })
}

pub(crate) fn problem_content(value: &WireProblemAggregate) -> Value {
    serde_json::json!({
        "subject": value.subject,
        "tags": value.tags,
        "note": value.note,
        "status": value.status,
        "timeLimitSeconds": value.time_limit_seconds,
        "assets": value.assets,
    })
}

pub(crate) fn export_content(value: &WireExportSnapshot) -> Value {
    serde_json::json!({
        "title": value.title,
        "problemIds": value.problem_ids,
        "configuration": value.configuration,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        FieldDecision, MergeAction, merge_export_versions, merge_field, merge_problem_versions,
        merge_profile_versions,
    };
    use crate::modules::sync_store::{WireExportSnapshot, WireProblemAggregate, WireProfile};

    fn profile(name: &str, revision: i64, updated_at_utc_ms: i64) -> WireProfile {
        WireProfile {
            id: "profile-1".to_owned(),
            name: name.to_owned(),
            revision,
            created_at_utc_ms: 1,
            updated_at_utc_ms,
        }
    }

    fn problem(subject: &str, revision: i64, updated_at_utc_ms: i64) -> WireProblemAggregate {
        WireProblemAggregate {
            id: "problem-1".to_owned(),
            profile_id: "profile-1".to_owned(),
            subject: subject.to_owned(),
            tags: vec!["重点".to_owned()],
            note: "note".to_owned(),
            status: "active".to_owned(),
            time_limit_seconds: Some(60),
            assets: Vec::new(),
            revision,
            created_at_utc_ms: 1,
            updated_at_utc_ms,
        }
    }

    #[test]
    fn missing_local_versions_apply_remote_entities_without_rewriting_them() {
        let profile = profile("Remote", 3, 30);
        let problem = problem("Remote", 4, 40);
        let export = WireExportSnapshot {
            id: "export-1".to_owned(),
            profile_id: "profile-1".to_owned(),
            title: "Remote".to_owned(),
            problem_ids: vec!["problem-1".to_owned()],
            configuration: json!({ "layout": "worksheet" }),
            revision: 5,
            created_at_utc_ms: 50,
        };

        assert!(matches!(
            merge_profile_versions(None, None, &profile, 99).unwrap(),
            MergeAction::ApplyRemote(value) if value == profile
        ));
        assert!(matches!(
            merge_problem_versions(None, None, &problem, 99).unwrap(),
            MergeAction::ApplyRemote(value) if value == problem
        ));
        assert!(matches!(
            merge_export_versions(None, None, &export).unwrap(),
            MergeAction::ApplyRemote(value) if value == export
        ));
    }

    #[test]
    fn field_merge_truth_table_is_deterministic() {
        let base = json!("base");
        assert!(matches!(
            merge_field(Some(&base), &json!("local"), &base),
            FieldDecision::Value(value) if value == json!("local")
        ));
        assert!(matches!(
            merge_field(Some(&base), &base, &json!("remote")),
            FieldDecision::Value(value) if value == json!("remote")
        ));
        assert!(matches!(
            merge_field(Some(&base), &json!("same"), &json!("same")),
            FieldDecision::Value(value) if value == json!("same")
        ));
        assert!(matches!(
            merge_field(Some(&base), &json!("local"), &json!("remote")),
            FieldDecision::Conflict { .. }
        ));
    }

    #[test]
    fn profile_local_only_change_is_revised_and_enqueued() {
        let base = profile("Base", 1, 10);
        let local = profile("Local", 2, 20);
        let remote = profile("Base", 2, 30);

        let action = merge_profile_versions(Some(&local), Some(&base), &remote, 40).unwrap();

        assert!(matches!(
            action,
            MergeAction::ApplyMergedAndEnqueue(value)
                if value.name == "Local" && value.revision == 3 && value.updated_at_utc_ms == 40
        ));
    }

    #[test]
    fn problem_divergent_field_keeps_local_value_and_records_conflict() {
        let base = problem("Base", 1, 10);
        let local = problem("Local", 2, 20);
        let remote = problem("Remote", 3, 30);

        let action = merge_problem_versions(Some(&local), Some(&base), &remote, 40).unwrap();

        assert!(matches!(
            action,
            MergeAction::ApplyPartialWithConflicts { value, conflicts }
                if value.subject == "Local"
                    && value.revision == 4
                    && value.updated_at_utc_ms == 40
                    && conflicts.len() == 1
                    && conflicts[0].field_name == "subject"
                    && conflicts[0].base_revision == 1
        ));
    }
}
