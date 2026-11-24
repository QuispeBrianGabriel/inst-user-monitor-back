//! Capturas de la lista de seguidores de una cuenta monitoreada.
//!
//! El usuario copia la lista en Instagram y la pega: el texto se lee entero
//! (submódulo `parser`) y, si algo no encaja, no se guarda nada. Cada captura
//! se compara con la anterior (submódulo `compare`) para mostrar qué cambió.

mod compare;
mod parser;

use std::{collections::HashMap, result::Result};

use loco_rs::prelude::*;
use sea_orm::{FromQueryResult, QueryOrder, QuerySelect};
use serde::Deserialize;

pub use self::{
    compare::{Changes, Rename},
    parser::{Entry, MAX_ENTRIES, ParseError},
};
pub use super::_entities::account_snapshots::{self, ActiveModel, Column, Entity, Model};

/// Cuerpo de `POST /accounts/{id}/snapshots`: el texto pegado.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotParams {
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error(transparent)]
    InvalidFormat(#[from] ParseError),
    #[error(transparent)]
    Model(#[from] ModelError),
}

impl From<DbErr> for SnapshotError {
    fn from(error: DbErr) -> Self {
        Self::Model(error.into())
    }
}

/// Datos de una captura sin su lista, para el historial.
#[derive(Debug, Clone, FromQueryResult)]
pub struct SnapshotInfo {
    pub id: Uuid,
    pub account_id: Uuid,
    pub total: i32,
    pub captured_at: DateTimeWithTimeZone,
}

/// Historial de capturas de una cuenta, con las dos últimas completas para
/// compararlas.
#[derive(Debug, Clone)]
pub struct Timeline {
    /// De la más reciente a la más antigua; nunca vacío.
    pub history: Vec<SnapshotInfo>,
    /// Cuentas de la última captura.
    pub current: Vec<Entry>,
    /// Cuentas de la penúltima, si la hay.
    pub previous: Option<Vec<Entry>>,
}

impl Timeline {
    #[must_use]
    pub fn latest(&self) -> &SnapshotInfo {
        &self.history[0]
    }

    #[must_use]
    pub fn previous_info(&self) -> Option<&SnapshotInfo> {
        self.history.get(1)
    }

    /// Cambios de la última captura respecto de la anterior.
    #[must_use]
    pub fn changes(&self) -> Option<Changes> {
        self.previous
            .as_deref()
            .map(|previous| Changes::between(previous, &self.current))
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut this = self;
        if insert && !this.id.is_set() {
            this.id = ActiveValue::Set(Uuid::now_v7());
        }
        Ok(this)
    }
}

impl Model {
    /// Lee el texto pegado y lo guarda como la última captura de seguidores.
    /// La cuenta tiene que ser del usuario: compruébalo antes.
    ///
    /// # Errors
    ///
    /// `InvalidFormat` si el texto no es una lista de Instagram (y entonces
    /// no se guarda nada), o un error inesperado de la base.
    pub async fn create(
        db: &DatabaseConnection,
        account_id: Uuid,
        params: &SnapshotParams,
    ) -> Result<Self, SnapshotError> {
        let entries = parser::parse(&params.text)?;
        let total = i32::try_from(entries.len()).map_err(|_| ParseError::TooManyEntries)?;
        let entries = serde_json::to_value(entries).map_err(ModelError::wrap)?;
        Ok(account_snapshots::ActiveModel {
            account_id: ActiveValue::Set(account_id),
            total: ActiveValue::Set(total),
            entries: ActiveValue::Set(entries),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }

    /// Cuentas de la captura.
    ///
    /// # Errors
    ///
    /// Si la columna `entries` no tiene el formato esperado.
    pub fn entries(&self) -> ModelResult<Vec<Entry>> {
        serde_json::from_value(self.entries.clone()).map_err(ModelError::wrap)
    }

    /// Busca una captura de la cuenta `account_id`.
    ///
    /// # Errors
    ///
    /// `ModelError::EntityNotFound` si no existe o es de otra cuenta, o un
    /// error de la base.
    pub async fn find_for_account(
        db: &DatabaseConnection,
        account_id: Uuid,
        id: Uuid,
    ) -> ModelResult<Self> {
        Entity::find_by_id(id)
            .filter(Column::AccountId.eq(account_id))
            .one(db)
            .await?
            .ok_or(ModelError::EntityNotFound)
    }

    /// Borra la captura (p. ej. si se pegó la lista equivocada).
    ///
    /// # Errors
    ///
    /// Un error de la base.
    pub async fn remove(self, db: &DatabaseConnection) -> ModelResult<()> {
        self.delete(db).await?;
        Ok(())
    }

    /// Historial de capturas de las cuentas indicadas, por cuenta. Las
    /// cuentas sin capturas no aparecen.
    ///
    /// Dos consultas: el historial sin las listas (ligero) y, completas,
    /// solo las dos últimas capturas de cada cuenta.
    ///
    /// # Errors
    ///
    /// Un error de la base o una captura con un formato inesperado.
    pub async fn timelines(
        db: &DatabaseConnection,
        account_ids: &[Uuid],
    ) -> ModelResult<HashMap<Uuid, Timeline>> {
        if account_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let infos = Entity::find()
            .select_only()
            .columns([
                Column::Id,
                Column::AccountId,
                Column::Total,
                Column::CapturedAt,
            ])
            .filter(Column::AccountId.is_in(account_ids.iter().copied()))
            .order_by_desc(Column::CapturedAt)
            .order_by_desc(Column::Id)
            .into_model::<SnapshotInfo>()
            .all(db)
            .await?;

        let mut histories: HashMap<Uuid, Vec<SnapshotInfo>> = HashMap::new();
        for info in infos {
            histories.entry(info.account_id).or_default().push(info);
        }

        let recent_ids = histories
            .values()
            .flat_map(|history| history.iter().take(2).map(|info| info.id));
        let mut recent: HashMap<Uuid, Vec<Entry>> = HashMap::new();
        for snapshot in Entity::find()
            .filter(Column::Id.is_in(recent_ids))
            .all(db)
            .await?
        {
            recent.insert(snapshot.id, snapshot.entries()?);
        }

        Ok(histories
            .into_iter()
            .map(|(account_id, history)| {
                let current = recent.remove(&history[0].id).unwrap_or_default();
                let previous = history.get(1).and_then(|info| recent.remove(&info.id));
                let timeline = Timeline {
                    history,
                    current,
                    previous,
                };
                (account_id, timeline)
            })
            .collect())
    }
}
