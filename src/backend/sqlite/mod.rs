mod delta;

use std::{
    error::{self, Error},
    ops::Deref,
};

use crate::{
    artifact::{Artifact, ConsumerError, ContentId, ScriptConsumer, ScriptProcessingError},
    util::empty_database_version,
    SQIGL_VERSION,
};
use log::{debug, error, info, trace, warn};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use semver::Version;
use thiserror::Error;

use self::delta::delta;

use super::{Backend, GeneratedMigration, SqiglState};

impl ConsumerError for rusqlite::Error {}

fn get_state<Db: Deref<Target = rusqlite::Connection>>(
    db: &Db,
) -> Result<SqiglState, rusqlite::Error> {
    Ok(db
        .prepare_cached(include_str!("sql/select_state.sql"))?
        .query_row([], |r| r.try_into())?)
}

pub struct SqliteBackend {
    db: rusqlite::Connection,
}
impl SqliteBackend {
    pub fn new(db: rusqlite::Connection) -> Self {
        Self { db }
    }
    pub fn local() -> Result<Self, rusqlite::Error> {
        Ok(Self {
            db: rusqlite::Connection::open_in_memory()?,
        })
    }
}
impl Backend for SqliteBackend {
    type Error = rusqlite::Error;

    fn install(&mut self) -> Result<SqiglState, Self::Error> {
        info!("Installing sqigl onto databse");
        let mut tx = self.db.transaction()?;
        tx.execute_batch(include_str!("sql/schema.sql"))?;
        tx.prepare(include_str!("sql/initialize_state.sql"))?
            .execute([SQIGL_VERSION])?;
        let state = get_state(&tx)?;
        tx.commit()?;

        Ok(state)
    }
    fn open(&mut self) -> Result<SqiglState, Self::Error> {
        if let Ok(state) = get_state(&&self.db) {
            Ok(state)
        } else {
            warn!("sqigl is not installed on this database; installing");
            let state = self.install()?;
            Ok(state)
        }
    }

    fn apply<A: Artifact>(
        &mut self,
        artifact: &A,
    ) -> Result<SqiglState, ScriptProcessingError<Self::Error>> {
        info!("Applying artifact.");
        struct Consumer<'a> {
            version: &'a Version,
            tx: rusqlite::Transaction<'a>,
        }
        impl ScriptConsumer for Consumer<'_> {
            type Error = rusqlite::Error;

            fn accept(
                &mut self,
                script: &str,
            ) -> Result<(), ScriptProcessingError<rusqlite::Error>> {
                trace!("Running a script.");
                self.tx.execute_batch(script)?;
                Ok(())
            }

            fn commit(self, id: ContentId) -> Result<(), ScriptProcessingError<rusqlite::Error>> {
                debug!("Committing migration.");
                // NB: An artifact may be applied multiple times. Our artifact's row may
                // already exist.
                let artifact_pk: i64 = self
                    .tx
                    .prepare(include_str!("sql/get_artifact_by_id.sql"))?
                    .query_row([id.unwrap()], |r| r.get("pk"))?;
                let prev_pk: Option<i64> =
                    self.tx
                        .query_row("select head from sqigl_internal_state", [], |r| {
                            r.get("head")
                        })?;
                let head_pk: i64 = self
                    .tx
                    .prepare(include_str!("sql/append_history.sql"))?
                    .query_row((prev_pk, artifact_pk, self.version.to_string()), |r| {
                        r.get::<_, i64>("pk")
                    })?;
                self.tx
                    .prepare("update sqigl_internal_state set head = ?1")?
                    .execute([head_pk])?;
                self.tx.commit()?;
                debug!("Migration committed.");
                Ok(())
            }
        }

        // To protect against accidentally running two instances of sqigl at once
        // (eg in a flawed CI script), we must ensure:
        // - All instances run serially
        // - The version is compatible at the start of our transaction
        debug!("Opening artifact transaction.");
        let tx = Transaction::new(&mut self.db, TransactionBehavior::Exclusive)?;
        let state = get_state(&tx)?;
        if !artifact.compatible(&state.project_version) {
            error!("Migration aborted: Incompatible");
            return Err(ScriptProcessingError::Incompatible);
        }

        let version = artifact.version();
        let consumer = Consumer { version, tx };
        artifact.scripts(consumer)?;

        let state = get_state(&&self.db)?;
        Ok(state)
    }

    fn check<A: Artifact>(
        &mut self,
        artifact: &A,
    ) -> Result<(), ScriptProcessingError<Self::Error>> {
        info!("Checking artifact.");

        let _ = self.open()?;
        struct Consumer<'a> {
            tx: rusqlite::Transaction<'a>,
        }
        impl ScriptConsumer for Consumer<'_> {
            type Error = rusqlite::Error;

            fn accept(
                &mut self,
                script: &str,
            ) -> Result<(), ScriptProcessingError<rusqlite::Error>> {
                trace!("Running a script.");
                self.tx.execute_batch(script)?;
                Ok(())
            }

            fn commit(self, id: ContentId) -> Result<(), ScriptProcessingError<rusqlite::Error>> {
                trace!("Done checking, rolling back");
                self.tx.rollback()?;
                Ok(())
            }
        }

        let tx = self.db.transaction()?;
        let state = get_state(&tx)?;
        if !artifact.compatible(&state.project_version) {
            error!("Migration aborted: Incompatible");
            return Err(ScriptProcessingError::Incompatible);
        }

        let version = artifact.version();
        let consumer = Consumer { tx };
        artifact.scripts(consumer)?;

        Ok(())
    }

    fn generate_migration<A1: Artifact, A2: Artifact>(
        &mut self,
        from_schema: &A1,
        to_schema: &A2,
    ) -> anyhow::Result<impl Artifact> {
        let from_db = Connection::open_in_memory()?;
        from_db.execute_batch(&from_schema.to_string())?;
        let to_db = Connection::open_in_memory()?;
        to_db.execute_batch(&to_schema.to_string())?;
        let statements = delta(from_db, to_db)?;

        let from = crate::util::from_minor_version(from_schema.version());
        let to = to_schema.version().clone();
        Ok(GeneratedMigration {
            from,
            to,
            statements,
        })
    }
}
impl Default for SqliteBackend {
    fn default() -> Self {
        Self {
            db: rusqlite::Connection::open_in_memory().unwrap(),
        }
    }
}

impl TryFrom<&rusqlite::Row<'_>> for SqiglState {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row) -> Result<Self, Self::Error> {
        Ok(Self {
            project_version: row
                .get::<_, Option<String>>("project_version")?
                .map(|s| {
                    s.parse::<Version>()
                        .expect("Failed to parse semver in project_version")
                })
                .unwrap_or_else(empty_database_version),
            sqigl_version: row
                .get::<_, String>("sqigl_version")?
                .parse()
                .expect("Failed to parse semver in sqigl_version"),
        })
    }
}
