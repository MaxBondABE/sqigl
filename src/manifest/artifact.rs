use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
};

use log::{error, warn};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use toml_edit::DocumentMut;

use crate::{
    manifest::{read_toml, MANIFEST_FILENAME},
    util::{new_table, new_table_arr, normalize_version, replace_file},
};

use super::{project::ProjectInfo, ReadTomlError};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub migrations: Vec<Migration>,
}
impl ArtifactManifest {
    pub const KEY: &'static str = "artifact";
}

#[derive(Clone, Debug, Default)]
pub struct ArtifactInfo {
    pub migrations: Vec<Migration>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Migration {
    pub script: PathBuf,
    pub from: VersionReq,
    pub to: Version,
}
impl Migration {
    pub fn insert(&self, table: &mut toml_edit::Table) {
        table["script"] = self.script.to_str().unwrap().into();
        table["from"] = self.from.to_string().into();
        table["to"] = self.to.to_string().into();
    }
}
impl Migration {
    pub const KEY: &str = "migrations";
}
impl From<Migration> for toml_edit::Table {
    fn from(value: Migration) -> Self {
        let mut tbl = toml_edit::Table::new();
        value.insert(&mut tbl);
        tbl
    }
}

pub fn open_artifact(directory: PathBuf) -> Result<ArtifactInfo, OpenError> {
    debug_assert!(directory.is_dir(), "Directory does not exist or is a file.");
    debug_assert!(
        directory == directory.canonicalize().unwrap(),
        "The directory must be canonical to ensure that all paths in the output are canonical."
    );
    let manifest_path = directory.join(MANIFEST_FILENAME);
    if manifest_path.is_file() {
        let (manifest) = read_toml::<ArtifactManifest>(&manifest_path)?;
        for migration in manifest.migrations.iter() {
            let script = migration.script.to_str().unwrap();
            if script.contains("/") {
                return Err(OpenError::InvalidScript(script.to_string()));
            }
        }
        let ArtifactManifest { migrations, .. } = manifest;
        Ok(ArtifactInfo { migrations })
    } else {
        Err(OpenError::NotFound(directory))
    }
}

#[derive(Error, Debug)]
pub enum OpenError {
    #[error("No project manifest was found in {0} or any of it's ancestors.")]
    NotFound(PathBuf),
    #[error("Invalid script path {0}: Must not contain /")]
    InvalidScript(String),
    #[error("I/O Error: {0}")]
    Io(#[from] io::Error),
    #[error("TOML syntax error: {0}")]
    SyntaxError(toml::de::Error),
    #[error("Invalid manifest: {0}")]
    Invalid(toml::de::Error),
}
impl From<ReadTomlError> for OpenError {
    fn from(value: ReadTomlError) -> Self {
        match value {
            ReadTomlError::Io(e) => e.into(),
            ReadTomlError::SyntaxError(e) => Self::SyntaxError(e),
            ReadTomlError::Invalid(e) => Self::Invalid(e),
        }
    }
}

pub fn update_artifact_migration(
    migration: Migration,
    artifact_directory: PathBuf,
) -> Result<(), UpdateMigrationError> {
    let path = artifact_directory.join(MANIFEST_FILENAME);
    let mut document: DocumentMut = {
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            content.parse()?
        } else {
            DocumentMut::new()
        }
    };
    {
        let migrations = document
            .entry(Migration::KEY)
            .or_insert_with(new_table_arr)
            .as_array_of_tables_mut()
            .ok_or_else(|| UpdateMigrationError::InvalidValue(Migration::KEY.to_string()))?;

        let mut iterator = migrations.iter_mut();
        if let Some(m) = iterator.find(|m| {
            if let Some(v) = m.get("script") {
                if let Some(s) = v.as_str() {
                    OsStr::new(s) == migration.script.as_os_str()
                } else {
                    error!("Migration script was not a string");
                    false
                }
            } else {
                error!("Migration did not contain script");
                false
            }
        }) {
            migration.insert(m);
        } else {
            drop(iterator); // We need to explicitly save the iterator to a variable and drop it,
                            // because temporaries are dropped at the end of the scope, which would
                            // prevent us from borrowing mut here.
            migrations.push(migration.into());
        }
    }

    replace_file(&document.to_string(), &path)?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum UpdateMigrationError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("TOML syntax error: {0}")]
    SyntaxError(#[from] toml_edit::TomlError),
    #[error("Invalid value in key {0}")]
    InvalidValue(String),
}

pub fn update_migration_versions(
    old_version: &Version,
    new_version: &Version,
    info: &ProjectInfo,
) -> Result<(), UpdateVersionsError> {
    let artifacts_dir = info.artifacts_dir();
    let old_module = artifacts_dir.join(normalize_version(&old_version).to_string());
    let new_module = artifacts_dir.join(normalize_version(&new_version).to_string());
    if new_module.exists() {
        return Err(UpdateVersionsError::AlreadyExists(new_version.clone()));
    }
    match (old_module.exists(), new_module.exists()) {
        (true, false) => fs::rename(&old_module, &new_module)?,
        (false, false) => fs::create_dir_all(&new_module)?,
        (true, true) => {
            todo!()
        }
        (false, true) => {
            todo!()
        }
    };

    let manifest_path = new_module.join(MANIFEST_FILENAME);
    if !manifest_path.exists() {
        warn!("No manifest found");
        return Ok(());
    }

    let content = fs::read_to_string(&manifest_path)?;
    let mut document: DocumentMut = content.parse()?;
    drop(content);
    let mut migrations = document
        .entry(Migration::KEY)
        .or_insert_with(new_table_arr)
        .as_array_of_tables_mut()
        .ok_or_else(|| UpdateVersionsError::InvalidValue(Migration::KEY.to_string()))?;
    let new_version_str = new_version.to_string();
    for migration in migrations.iter_mut() {
        if let Some(to) = migration.get_mut("to") {
            if let Some(s) = to.as_str() {
                if let Ok(version) = s.parse::<Version>() {
                    if version == *old_version {
                        *to = new_version_str.clone().into();
                    }
                } else {
                    error!(
                        "Could not parse `to` value in migration {}",
                        migration
                            .position()
                            .expect("All migrations were parsed, and so have position")
                    );
                }
            } else {
                error!(
                    "Unable to process `to` value in migration {}",
                    migration
                        .position()
                        .expect("All migrations were parsed, and so have position")
                );
            }
        } else {
            error!(
                "No `to` value in migration {}",
                migration
                    .position()
                    .expect("All migrations were parsed, and so have position")
            );
        }
    }

    replace_file(&document.to_string(), &manifest_path)?;
    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum UpdateVersionsError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("Artifact module for version {0} already exists")]
    AlreadyExists(Version),
    #[error("Could not read manifest: {0}")]
    Invalid(#[from] toml_edit::TomlError),
    #[error("Invalid value in key {0}")]
    InvalidValue(String),
}
