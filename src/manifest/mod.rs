pub mod artifact;
pub mod module;
pub mod project;

use crate::{
    artifact::ContentId,
    util::{empty_database_version, new_project_version, new_table},
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
use toml_edit::DocumentMut;

pub const MANIFEST_FILENAME: &str = "sqigl.toml";
pub const SOURCE_DIRECTORY: &str = "src";
pub const ARTIFACTS_DIRECTORY: &str = "artifacts";

pub fn read_toml<'de, T: Deserialize<'de>>(path: &Path) -> Result<T, ReadTomlError> {
    let content = fs::read_to_string(path)?;
    let table: toml::Table = toml::from_str(&content)?;
    match table.try_into() {
        Ok(x) => Ok(x),
        Err(e) => Err(ReadTomlError::Invalid(e)),
    }
}

pub fn maybe_read_toml<'de, T: Deserialize<'de>>(
    path: &Path,
    key: &str,
) -> Result<Option<T>, ReadTomlError> {
    let content = fs::read_to_string(path)?;
    let table: toml::Table = toml::from_str(&content)?;
    if table.contains_key(key) {
        match table.try_into() {
            Ok(x) => Ok(x),
            Err(e) => Err(ReadTomlError::Invalid(e)),
        }
    } else {
        Ok(None)
    }
}

#[derive(Debug, Error)]
pub enum ReadTomlError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    SyntaxError(#[from] toml::de::Error),
    #[error("{0}")]
    Invalid(toml::de::Error),
}
