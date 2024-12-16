use std::{fs, io, num::NonZeroU16, path::PathBuf};

use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use toml_edit::DocumentMut;

use crate::{
    manifest::{maybe_read_toml, ARTIFACTS_DIRECTORY, MANIFEST_FILENAME, SOURCE_DIRECTORY},
    util::{empty_database_version, new_project_version, new_table, replace_file},
};

use super::ReadTomlError;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub project: Project,
    pub database: Database,
}
impl ProjectManifest {
    pub const KEY: &'static str = "project";

    pub fn new(title: String, database: Database) -> Self {
        ProjectManifest {
            project: Project {
                version: new_project_version(),
                title,
            },
            database,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub version: Version,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct ProjectInfo {
    pub project: Project,
    pub database: Database,
    pub root: PathBuf,
}
impl ProjectInfo {
    pub fn project_manifest(&self) -> PathBuf {
        self.root.join(MANIFEST_FILENAME)
    }
    pub fn source_dir(&self) -> PathBuf {
        self.root.join(SOURCE_DIRECTORY)
    }
    pub fn artifacts_dir(&self) -> PathBuf {
        self.root.join(ARTIFACTS_DIRECTORY)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "db", rename_all = "lowercase")]
pub enum Database {
    Postgres(PostgresDatabase),
    Sqlite(SqliteDatabase),
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct PostgresDatabase {
    pub hostname: Option<String>,
    pub port: Option<NonZeroU16>,
    pub username: Option<String>,
    pub database: Option<String>,
    pub certificate: Option<PathBuf>,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct SqliteDatabase {
    pub path: Option<PathBuf>,
}

pub fn open_project(directory: PathBuf) -> Result<ProjectInfo, OpenError> {
    debug_assert!(directory.is_dir(), "Directory does not exist or is a file.");
    debug_assert!(
        directory == directory.canonicalize().unwrap(),
        "The directory must be canonical to ensure that all returned paths are canonical."
    );
    for d in directory.ancestors() {
        let manifest_path = d.join(MANIFEST_FILENAME);
        if manifest_path.is_file() {
            if let Some(project_manifest) =
                maybe_read_toml::<ProjectManifest>(&manifest_path, ProjectManifest::KEY)?
            {
                if project_manifest.project.version <= empty_database_version() {
                    return Err(OpenError::InvalidVersion);
                }

                let root = manifest_path.parent().unwrap();
                return Ok(ProjectInfo {
                    root: root.to_path_buf(),
                    project: project_manifest.project,
                    database: project_manifest.database,
                });
            }
        }
    }

    Err(OpenError::NotFound(directory))
}

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("Version 0.0.0 is reserved for empty databases.")]
    InvalidVersion,
    #[error("No project manifest was found in {0} or any of it's ancestors.")]
    NotFound(PathBuf),
    #[error("I/O error: {0}")]
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

pub fn update_project_version(
    new_version: &Version,
    info: &ProjectInfo,
) -> Result<(), UpdateVersionError> {
    let path = info.project_manifest();
    if !path.is_file() {
        return Err(UpdateVersionError::NotFound);
    }
    let content = fs::read_to_string(&path)?;
    let mut document: DocumentMut = content.parse()?;
    drop(content);

    document
        .entry(ProjectManifest::KEY)
        .or_insert_with(new_table)
        .as_table_mut()
        .ok_or_else(|| UpdateVersionError::InvalidValue(ProjectManifest::KEY.to_string()))?
        ["version"] = new_version.to_string().into();

    let new_manifest = document.to_string();
    replace_file(&new_manifest, &path)?;
    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum UpdateVersionError {
    #[error("The project manifest was not found.")]
    NotFound,
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Could not read manifest: {0}")]
    Invalid(#[from] toml_edit::TomlError),
    #[error("Invalid value in key {0}")]
    InvalidValue(String),
}
