use std::{collections::BTreeSet, error, fmt};

use anyhow::anyhow;
use log::{debug, info};
use semver::Version;
use thiserror::Error;

use crate::{
    artifact::{Artifact, ConsumerError, ScriptProcessingError},
    backend::{Backend, SqiglState},
    manifest::{artifact::open_artifact, project::ProjectInfo},
    migration::MigrationSet,
    util::empty_database_version,
};

pub fn apply_artifact<Db: Backend, A: Artifact>(
    mut database: Db,
    artifact: A,
) -> anyhow::Result<SqiglState>
where
    <Db as Backend>::Error: Send + Sync + 'static,
{
    info!("Applying artifact");

    let state = database.open()?;
    if !artifact.compatible(&state.project_version) {
        return Err(anyhow!(
            "Cannot apply: The database is not compatible with this artifact."
        ));
    }
    Ok(database.apply(&artifact)?)
}

pub fn apply_version<Db: Backend>(
    version: Version,
    info: &ProjectInfo,
    mut database: Db,
) -> anyhow::Result<()>
where
    <Db as Backend>::Error: Sync + Send + 'static,
{
    info!("Applying version {}", &version);

    let state = database.open()?;
    debug!("Current version: {}", &state.project_version);

    let migration_set = MigrationSet::open(info)?;
    if let Some(migration) = migration_set.get(&state.project_version, &version) {
        info!(
            "Applying migration {}",
            migration.script().to_str().unwrap()
        );

        database.apply(&migration)?;
        Ok(())
    } else {
        Err(anyhow!(
            "No saved migration for {} -> {}",
            &state.project_version,
            &version
        ))
    }
}

pub fn check_artifact<Db: Backend, A: Artifact>(artifact: A, mut database: Db) -> anyhow::Result<()>
where
    <Db as Backend>::Error: Send + Sync + 'static,
{
    database.check(&artifact)?;
    Ok(())
}