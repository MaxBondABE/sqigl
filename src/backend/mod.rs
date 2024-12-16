pub mod postgres;
pub mod sqlite;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt::Display};
use thiserror::Error;

use crate::artifact::{Artifact, ConsumerError, ScriptProcessingError};

pub trait Backend {
    type Error: Error;
    fn install(&mut self) -> Result<SqiglState, Self::Error>;
    fn open(&mut self) -> Result<SqiglState, Self::Error>;
    fn apply<A: Artifact>(
        &mut self,
        artifact: &A,
    ) -> Result<SqiglState, ScriptProcessingError<Self::Error>>;
    fn check<A: Artifact>(
        &mut self,
        artifact: &A,
    ) -> Result<(), ScriptProcessingError<Self::Error>>;
    fn generate_migration<A1: Artifact, A2: Artifact>(
        &mut self,
        from: &A1,
        to: &A2,
    ) -> anyhow::Result<impl Artifact>;
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SqiglState {
    pub project_version: Version,
    pub sqigl_version: Version,
}

pub trait SqlStatement {
    fn write_to(&self, buffer: &mut String);
}

pub struct GeneratedMigration<Stmt> {
    from: VersionReq,
    to: Version,
    statements: Vec<Stmt>,
}

impl<Stmt: SqlStatement> Artifact for GeneratedMigration<Stmt> {
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
        let mut buffer = String::with_capacity(1024);
        let mut hasher = Sha256::new();
        for stmt in self.statements.iter() {
            buffer.clear();
            stmt.write_to(&mut buffer);
            buffer.push('\n');
            hasher.update(&buffer);
            consumer.accept(&buffer)?;
        }

        let id = hasher.finalize().into();
        consumer.commit(id)?;

        Ok(id)
    }
}
