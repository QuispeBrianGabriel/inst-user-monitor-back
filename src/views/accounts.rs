//! Respuestas JSON de `/accounts`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::models::{
    account_snapshots::{self, Changes, Entry, ListKind, Relations, SnapshotInfo, Timeline},
    monitored_accounts,
};

#[derive(Debug, Serialize)]
pub struct AccountResponse {
    pub id: Uuid,
    pub username: String,
    pub full_name: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&monitored_accounts::Model> for AccountResponse {
    fn from(account: &monitored_accounts::Model) -> Self {
        Self {
            id: account.id,
            username: account.username.clone(),
            full_name: account.full_name.clone(),
            notes: account.notes.clone(),
            created_at: account.created_at.with_timezone(&Utc),
            updated_at: account.updated_at.with_timezone(&Utc),
        }
    }
}

/// Una captura, sin su lista.
#[derive(Debug, Serialize)]
pub struct SnapshotResponse {
    pub id: Uuid,
    /// `followers` o `following`.
    pub kind: String,
    pub total: i32,
    pub captured_at: DateTime<Utc>,
}

impl From<&SnapshotInfo> for SnapshotResponse {
    fn from(info: &SnapshotInfo) -> Self {
        Self {
            id: info.id,
            kind: info.kind.clone(),
            total: info.total,
            captured_at: info.captured_at.with_timezone(&Utc),
        }
    }
}

impl From<&account_snapshots::Model> for SnapshotResponse {
    fn from(snapshot: &account_snapshots::Model) -> Self {
        Self {
            id: snapshot.id,
            kind: snapshot.kind.clone(),
            total: snapshot.total,
            captured_at: snapshot.captured_at.with_timezone(&Utc),
        }
    }
}

/// Estado de una lista en el listado de cuentas: la última captura y cuánto
/// cambió respecto de la anterior (`None` si es la primera).
#[derive(Debug, Serialize)]
pub struct ListSummary {
    pub latest: SnapshotResponse,
    pub total_delta: Option<i64>,
    pub added: Option<usize>,
    pub removed: Option<usize>,
}

impl From<&Timeline> for ListSummary {
    fn from(timeline: &Timeline) -> Self {
        let changes = timeline.changes();
        Self {
            latest: timeline.latest().into(),
            total_delta: changes.as_ref().map(|changes| changes.total_delta),
            added: changes.as_ref().map(|changes| changes.added.len()),
            removed: changes.as_ref().map(|changes| changes.removed.len()),
        }
    }
}

/// Elemento de `GET /accounts`.
#[derive(Debug, Serialize)]
pub struct AccountSummary {
    pub account: AccountResponse,
    pub followers: Option<ListSummary>,
    pub following: Option<ListSummary>,
}

impl AccountSummary {
    #[must_use]
    pub fn new(account: &monitored_accounts::Model, timelines: &[Timeline]) -> Self {
        let summary = |kind| find(timelines, kind).map(ListSummary::from);
        Self {
            account: account.into(),
            followers: summary(ListKind::Followers),
            following: summary(ListKind::Following),
        }
    }
}

/// Una lista en el detalle de la cuenta: la última captura completa, sus
/// cambios respecto de la anterior y el historial de capturas.
#[derive(Debug, Serialize)]
pub struct ListReport {
    pub latest: SnapshotResponse,
    pub previous: Option<SnapshotResponse>,
    pub entries: Vec<Entry>,
    pub changes: Option<Changes>,
    /// De la más reciente a la más antigua.
    pub history: Vec<SnapshotResponse>,
}

impl From<&Timeline> for ListReport {
    fn from(timeline: &Timeline) -> Self {
        Self {
            latest: timeline.latest().into(),
            previous: timeline.previous_info().map(SnapshotResponse::from),
            entries: timeline.current.clone(),
            changes: timeline.changes(),
            history: timeline
                .history
                .iter()
                .map(SnapshotResponse::from)
                .collect(),
        }
    }
}

/// `GET /accounts/{id}`.
#[derive(Debug, Serialize)]
pub struct AccountDetail {
    pub account: AccountResponse,
    pub followers: Option<ListReport>,
    pub following: Option<ListReport>,
    /// Solo cuando hay capturas de las dos listas.
    pub relations: Option<Relations>,
}

impl AccountDetail {
    #[must_use]
    pub fn new(account: &monitored_accounts::Model, timelines: &[Timeline]) -> Self {
        let followers = find(timelines, ListKind::Followers);
        let following = find(timelines, ListKind::Following);
        Self {
            account: account.into(),
            followers: followers.map(ListReport::from),
            following: following.map(ListReport::from),
            relations: followers.zip(following).map(|(followers, following)| {
                Relations::between(&followers.current, &following.current)
            }),
        }
    }
}

fn find(timelines: &[Timeline], kind: ListKind) -> Option<&Timeline> {
    timelines.iter().find(|timeline| timeline.kind == kind)
}
