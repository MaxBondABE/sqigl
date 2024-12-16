use std::collections::HashSet;

use crate::{
    artifact::Artifact,
    backend::{GeneratedMigration, SqlStatement},
};

pub fn delta(
    mut from_db: impl postgres::GenericClient,
    mut to_db: impl postgres::GenericClient,
) -> anyhow::Result<Vec<Statement>> {
    unimplemented!("Generated migrations for postgres are not yet implemented")
}

pub enum Statement {}
impl SqlStatement for Statement {
    fn write_to(&self, buffer: &mut String) {
        match self {
            _ => (),
        }
    }
}
