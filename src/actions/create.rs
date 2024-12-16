use crate::{
    arguments::DatabaseKind,
    backend::Backend,
    manifest::{
        self,
        artifact::update_artifact_migration,
        project::{update_project_version, ProjectInfo, ProjectManifest},
        ARTIFACTS_DIRECTORY, SOURCE_DIRECTORY,
    },
    migration::{save_migration, MigrationSet},
    util::normalize_version,
};
use anyhow::anyhow;
use log::info;
use semver::{Prerelease, Version};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};
use tempfile::tempdir;
use toml_edit::DocumentMut;

use super::build::SQL_EXTENSION;

pub const PATCH_FILENAME_PREFIX: &str = "patch_";

pub fn create_project(
    manifest_path: PathBuf,
    title: String,
    database: DatabaseKind,
) -> Result<(), anyhow::Error> {
    info!("Creating new project");

    if manifest_path.exists() {
        return Err(anyhow!("project manifest already exists"));
    }
    let root = manifest_path.parent().unwrap();
    fs::create_dir_all(root)?;
    fs::create_dir(root.join(SOURCE_DIRECTORY))?;
    fs::create_dir(root.join(ARTIFACTS_DIRECTORY))?;

    let manifest = ProjectManifest::new(title, database.into());
    let mut f = File::create_new(manifest_path)?;
    f.write_all(toml::to_string(&manifest)?.as_bytes())?;

    Ok(())
}

pub fn new_feature(title: String, info: ProjectInfo) -> anyhow::Result<Version> {
    info!("Creating new feature version");

    if !info.project.version.pre.is_empty() {
        return Err(anyhow!(
            "Cannot create new feature version: Already on a feature version"
        ));
    }
    let mut new_version = info.project.version.clone();
    new_version.minor = info.project.version.minor + 1;
    new_version.pre = Prerelease::new(&title)?;

    update_project_version(&new_version, &info)?;
    Ok(new_version)
}

pub fn create_migration(from: Version, to: Version, info: &ProjectInfo) -> anyhow::Result<()> {
    info!("Creating new migration");

    let script_name = format!("from_{}.sql", from);
    let artifact_dir = info
        .artifacts_dir()
        .join(normalize_version(&to).to_string());
    let path = artifact_dir.join(&script_name);
    if path.exists() {
        Err(anyhow!(
            "Cannot create migration: {} already exists.",
            path.to_str().unwrap()
        ))
    } else {
        let _ = File::create_new(path)?;
        update_artifact_migration(
            crate::manifest::artifact::Migration {
                script: Path::new(&script_name).to_path_buf(),
                from: crate::util::from_minor_version(&from),
                to,
            },
            artifact_dir,
        )?;
        Ok(())
    }
}

pub fn generate_migration<Db: Backend>(
    from: Version,
    to: Version,
    database: &mut Db,
    info: &ProjectInfo,
) -> anyhow::Result<()> {
    info!("Generating migration");

    let migration_set = MigrationSet::open(info)?;
    let Some(from_schema) = migration_set.get_schema(&from) else {
        return Err(anyhow!("Could not find schema for {}", from));
    };
    let Some(to_schema) = migration_set.get_schema(&to) else {
        return Err(anyhow!("Could not find schema for {}", to));
    };
    let artifact = database.generate_migration(&from_schema, &to_schema)?;
    let title = format!("from_{}.sql", &from);

    save_migration(&title, artifact, info)?;
    Ok(())
}
