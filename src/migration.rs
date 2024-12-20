use std::{
    collections::BTreeMap,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use log::{debug, error, info, trace, warn};
use semver::{Version, VersionReq};
use sha2::{Digest, Sha256};

use crate::{
    actions::build::SQL_EXTENSION,
    artifact::{Artifact, ContentId},
    manifest::{
        self,
        artifact::{self, open_artifact, update_artifact_migration, ArtifactInfo},
        project::ProjectInfo,
        MANIFEST_FILENAME,
    },
    util::{empty_database_version, normalize_version, replace_artifact},
};

pub fn save_migration<A: Artifact>(
    title: &str,
    artifact: A,
    info: &ProjectInfo,
) -> anyhow::Result<PathBuf> {
    let (from, to) = artifact.spec();
    trace!("Saving migration {from} -> {to}");

    let version_dir = info
        .artifacts_dir()
        .join(normalize_version(&to).to_string());
    let script = Path::new(title).with_extension(SQL_EXTENSION);
    let script_path = version_dir.join(&script);
    replace_artifact(&artifact, &script_path)?;

    let migration = artifact::Migration { script, from, to };
    update_artifact_migration(migration, version_dir)?;

    Ok(script_path)
}

pub struct MigrationArtifact {
    from: VersionReq,
    to: Version,
    script: PathBuf,
}
impl MigrationArtifact {
    pub fn script(&self) -> &Path {
        &self.script
    }
}
impl Artifact for MigrationArtifact {
    fn compatible(&self, version: &Version) -> bool {
        self.from.matches(version)
    }
    fn version(&self) -> &Version {
        &self.to
    }
    fn spec(&self) -> (VersionReq, Version) {
        (self.from.clone(), self.to.clone())
    }

    fn scripts<C: crate::artifact::ScriptConsumer>(
        &self,
        mut consumer: C,
    ) -> Result<crate::artifact::ContentId, crate::artifact::ScriptProcessingError<C::Error>> {
        let code = fs::read_to_string(&self.script)?;
        let mut hasher = Sha256::new();
        hasher.update(&code);
        let id = hasher.finalize().into();

        consumer.accept(&code)?;
        consumer.commit(id)?;

        Ok(id)
    }
}

pub struct MigrationSet {
    entries: BTreeMap<Version, (PathBuf, Vec<artifact::Migration>)>,
}
impl MigrationSet {
    pub fn open(info: &ProjectInfo) -> Result<Self, MigrationSetError> {
        debug!("Enumerating migrations");

        let mut migrations: BTreeMap<Version, (PathBuf, Vec<_>)> = BTreeMap::default();
        let artifacts_dir = info.artifacts_dir();
        for child_res in artifacts_dir
            .read_dir()
            .map_err(|e| MigrationSetError::Io(artifacts_dir.clone(), e))?
        {
            let child = child_res.map_err(|e| MigrationSetError::Io(artifacts_dir.clone(), e))?;
            let path = child.path();
            let md = child
                .metadata()
                .map_err(|e| MigrationSetError::Io(path.clone(), e))?;
            if md.is_dir() {
                let manifest = open_artifact(path.clone())
                    .map_err(|e| MigrationSetError::OpenArtifact(path.clone(), e))?;
                for migration in manifest.migrations {
                    migrations
                        .entry(migration.to.clone())
                        .or_insert_with(|| (path.clone(), Default::default()))
                        .1
                        .push(migration)
                }
            } else {
                warn!("Ignoring {:?}: Not a directory", path)
            }
        }

        Ok(Self {
            entries: migrations,
        })
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Finds the latest released version (the highest version without prerelease information)
    pub fn latest_released_version(&self) -> Option<&Version> {
        self.entries.keys().rev().find(|k| k.pre.is_empty())
    }
    pub fn latest_compatible(&self, version: &Version) -> Option<MigrationArtifact> {
        for (path, migration_list) in self.entries.values().rev() {
            for migration in migration_list.iter() {
                if migration.from.matches(version) {
                    let artifact::Migration { script, from, to } = migration.clone();
                    return Some(MigrationArtifact {
                        from,
                        to,
                        script: path.join(script),
                    });
                }
            }
        }

        None
    }
    pub fn get(&self, from: &Version, to: &Version) -> Option<MigrationArtifact> {
        if let Some((path, candidates)) = self.entries.get(to) {
            candidates.iter().find(|m| m.from.matches(&from)).map(|m| {
                let artifact::Migration { script, from, to } = m.clone();
                MigrationArtifact {
                    from,
                    to,
                    script: path.join(script),
                }
            })
        } else {
            None
        }
    }
    pub fn get_schema(&self, version: &Version) -> Option<MigrationArtifact> {
        self.get(&empty_database_version(), version)
    }
}

struct MigrationSetEntry {
    path: PathBuf,
    migrations: Vec<artifact::Migration>,
}

#[derive(thiserror::Error, Debug)]
pub enum MigrationSetError {
    #[error("{0} {1}")]
    Io(PathBuf, io::Error),
    #[error("{0} {1}")]
    OpenArtifact(PathBuf, artifact::OpenError),
}
