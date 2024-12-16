use std::{io, path::PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::manifest::{read_toml, MANIFEST_FILENAME};

use super::ReadTomlError;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ModuleManifest {
    #[serde(default)]
    pub module: Module,
    #[serde(default)]
    pub scripts: Vec<Script>,
}
impl ModuleManifest {
    pub const KEY: &'static str = "module";
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Module {
    #[serde(default)]
    pub dependencies: Vec<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Script {
    pub script: PathBuf,
    #[serde(default)]
    pub dependencies: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct ModuleInfo {
    pub module: Module,
    pub scripts: Vec<Script>,
    pub path: PathBuf,
}
impl Eq for ModuleInfo {}
impl PartialEq for ModuleInfo {
    fn eq(&self, other: &Self) -> bool {
        self.path.eq(&other.path)
    }
}
impl Ord for ModuleInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.path.cmp(&other.path)
    }
}
impl PartialOrd for ModuleInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub fn open_module(directory: PathBuf) -> Result<ModuleInfo, OpenError> {
    debug_assert!(directory.is_dir(), "Directory does not exist or is a file.");
    debug_assert!(
        directory == directory.canonicalize().unwrap(),
        "The directory must be canonical to ensure that all paths in the output are canonical."
    );
    let manifest_path = directory.join(MANIFEST_FILENAME);
    if manifest_path.exists() {
        let manifest = read_toml::<ModuleManifest>(&manifest_path)?;
        for script in manifest.scripts.iter() {
            let path = script.script.to_str().unwrap();
            if path.contains("/") {
                return Err(OpenError::InvalidScript(path.to_string()));
            }
        }
        return Ok(ModuleInfo {
            module: manifest.module,
            scripts: manifest.scripts,
            path: directory,
        });
    }

    Ok(ModuleInfo {
        module: Default::default(),
        scripts: Default::default(),
        path: directory,
    })
}

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("Invalid script path {0}: Must not contain /")]
    InvalidScript(String),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("TOML syntax error: {0}")]
    SyntaxError(toml::de::Error),
    #[error("Could not read manifest: {0}")]
    Invalid(#[from] toml::de::Error),
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
