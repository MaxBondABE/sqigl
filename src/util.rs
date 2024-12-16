use std::{
    fs::{self, File},
    io::{self, Write},
    path::Path,
};

use semver::{BuildMetadata, Comparator, Op, Prerelease, Version, VersionReq};
use tempfile::tempdir;

use crate::{
    artifact::{Artifact, ContentId, NullConsumerError, ScriptProcessingError},
    backend::SqiglState,
};

/// Version `0.0.0`
pub fn empty_database_version() -> Version {
    Version {
        major: 0,
        minor: 0,
        patch: 0,
        pre: Prerelease::EMPTY,
        build: BuildMetadata::EMPTY,
    }
}

/// Version `0.1.0`
pub fn new_project_version() -> Version {
    Version {
        major: 0,
        minor: 1,
        patch: 0,
        pre: Prerelease::EMPTY,
        build: BuildMetadata::EMPTY,
    }
}

/// Requirement `=0.0.0`
pub fn from_empty_database() -> VersionReq {
    VersionReq {
        comparators: vec![Comparator {
            op: Op::Exact,
            major: 0,
            minor: Some(0),
            patch: Some(0),
            pre: Prerelease::EMPTY,
        }],
    }
}

/// For a version `a.b.c` returns requirement `=a.b`
pub fn from_minor_version(version: &Version) -> VersionReq {
    VersionReq {
        comparators: vec![Comparator {
            op: Op::Exact,
            major: version.major,
            minor: Some(version.minor),
            patch: None,
            pre: version.pre.clone(),
        }],
    }
}

/// For a version `a.b.c` returns requirement `=a.b.c`
pub fn from_patch_version(version: &Version) -> VersionReq {
    VersionReq {
        comparators: vec![Comparator {
            op: Op::Exact,
            major: version.major,
            minor: Some(version.minor),
            patch: Some(version.patch),
            pre: version.pre.clone(),
        }],
    }
}

/// Normalize version for use in artifact directories
pub fn normalize_version(version: &Version) -> Version {
    Version {
        patch: 0,
        pre: version.pre.clone(),
        build: BuildMetadata::EMPTY,
        ..*version
    }
}

pub fn new_table() -> toml_edit::Item {
    toml_edit::Item::Table(Default::default())
}

pub fn new_table_arr() -> toml_edit::Item {
    toml_edit::Item::ArrayOfTables(Default::default())
}

/// Write to a file atomically.
pub fn replace_file(content: &str, path: &Path) -> Result<(), io::Error> {
    let tmp_dir = tempdir()?;
    let tmp_path = tmp_dir.path().join("tmp");
    let mut f = File::create_new(&tmp_path)?;
    f.write_all(content.as_bytes())?;
    f.sync_data()?;
    drop(f);

    // This rename makes our write atomic.
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Write the contents of an artifact to a file atomically.
pub fn replace_artifact<A: Artifact>(
    artifact: &A,
    path: &Path,
) -> Result<ContentId, ReplaceArtifactError> {
    let tmp_dir = tempdir()?;
    let tmp_path = tmp_dir.path().join("tmp");
    let mut f = File::create_new(&tmp_path)?;
    let id = artifact.write_to(&mut f)?;
    f.sync_data()?;
    drop(f);

    // This rename makes our write atomic.
    fs::rename(&tmp_path, path)?;
    Ok(id)
}

#[derive(thiserror::Error, Debug)]
pub enum ReplaceArtifactError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Artiface(#[from] ScriptProcessingError<NullConsumerError>),
}
