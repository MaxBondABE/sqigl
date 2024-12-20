use std::{fs, io};

use crate::{
    actions::build::build_project,
    arguments::ReleaseLevel,
    backend::Backend,
    manifest::{
        artifact::update_migration_versions,
        project::{update_project_version, ProjectInfo},
        ARTIFACTS_DIRECTORY,
    },
    migration::{save_migration, MigrationSet},
    util::{empty_database_version, from_empty_database, normalize_version},
};

use anyhow::anyhow;
use log::{debug, error, info, warn};
use semver::{BuildMetadata, Version};

pub const SCHEMA_ARTIFACT_TITLE: &str = "schema";

pub fn save_project(info: &ProjectInfo) -> anyhow::Result<()> {
    info!(
        "Saving {} version {}",
        info.project.title, info.project.version
    );

    let version = info.project.version.clone();
    let artifacts_dir = info.artifacts_dir();
    let build = build_project(info)?;

    let normalized = normalize_version(&version);
    let version_dir = artifacts_dir.join(normalized.to_string());
    fs::create_dir_all(&version_dir)?;

    save_migration(SCHEMA_ARTIFACT_TITLE, build, info)?;

    info!("Save complete");
    Ok(())
}

pub fn release<Db: Backend>(
    level: ReleaseLevel,
    info: &ProjectInfo,
    mut database: Db,
) -> anyhow::Result<Version>
where
    <Db as Backend>::Error: Send + Sync + 'static,
{
    info!("Releasing project");
    if info.project.version.pre.is_empty() {
        warn!("Not on a a feature version");
    }

    // Validate the project is not broken before releasing.
    let mut built = match build_project(info) {
        Ok(x) => x,
        Err(e) => return Err(e.into()),
    };

    let old_version = info.project.version.clone();
    debug!("Current version: {}", &old_version);

    let migrations = MigrationSet::open(info)?;
    let latest_local = migrations
        .latest_released_version()
        .cloned()
        .unwrap_or(empty_database_version());
    drop(migrations);
    debug!("Latest local version: {}", &latest_local);

    let state = database.open()?;
    debug!("Latest remote version: {}", &state.project_version);

    let latest = latest_local.max(state.project_version);
    let new_version = level.release_version(&latest);
    info!("Assigned version {} to this release", &new_version);

    info!("Updating project manifest");
    update_project_version(&new_version, info)?;

    info!("Updating artifact manifest");
    update_migration_versions(&old_version, &new_version, info)?;

    info!("Saving project");
    built.set_version(&new_version);
    save_migration(SCHEMA_ARTIFACT_TITLE, built, info)?;

    info!("Released version {}", &new_version);
    Ok(new_version)
}
