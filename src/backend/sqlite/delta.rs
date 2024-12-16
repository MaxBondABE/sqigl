use std::{collections::HashSet, fmt::Write};

use crate::{
    backend::{Backend, GeneratedMigration, SqlStatement},
    util::{empty_database_version, from_minor_version},
    Artifact,
};
use log::info;
use rusqlite::Connection;
use semver::{Version, VersionReq};
use sha2::{Digest as _, Sha256};

use super::SqliteBackend;

pub fn delta(
    mut from_db: rusqlite::Connection,
    mut to_db: rusqlite::Connection,
) -> anyhow::Result<Vec<Statement>> {
    let mut statements = Vec::default();

    let from_tables = get_table_names(&mut from_db)?;
    let to_tables = get_table_names(&mut to_db)?;
    for tbl in from_tables.iter() {
        if !to_tables.contains(tbl) {
            info!("Table {} was deleted", tbl);
            statements.push(Statement::DropTable { name: tbl.clone() })
        }
    }
    for tbl in to_tables.iter() {
        if !from_tables.contains(tbl) {
            let code = to_db
                .prepare_cached(include_str!("sql/get_table_code.sql"))?
                .query_row([tbl], |row| Ok(row.get::<_, String>(0)?))?;
            statements.push(Statement::CreateTable { code })
        }
    }

    Ok(statements)
}

fn get_table_names(db: &mut Connection) -> anyhow::Result<HashSet<String>> {
    let mut output = HashSet::default();
    for name_res in db
        .prepare(include_str!("sql/table_names.sql"))?
        .query_map([], |row| Ok(row.get::<_, String>(0)?))?
    {
        output.insert(name_res?);
    }

    Ok(output)
}

pub enum Statement {
    DropTable { name: String },
    CreateTable { code: String },
}
impl SqlStatement for Statement {
    fn write_to(&self, buffer: &mut String) {
        match self {
            Statement::DropTable { name } => {
                buffer
                    .write_fmt(format_args!("DROP TABLE {};", name))
                    .unwrap();
            }
            Statement::CreateTable { code } => {
                buffer.push_str(&code);
                buffer.push(';');
            }
        }
    }
}
