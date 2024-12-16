#![allow(unused)]
#![deny(unused_must_use, clippy::dbg_macro)]

use crate::{
    actions::{apply::check_artifact, create::create_migration},
    arguments::{
        DatabaseCommand as DbCmd, MigrationCommands, ProjectCommands as ProjCmd, SqiglArguments,
        SqiglCommands as Cmd,
    },
    backend::Backend,
};
use actions::{
    apply::{apply_artifact, apply_version},
    build::build_project,
    create::{create_project, generate_migration, new_feature},
    save::{release, save_project},
};
use anyhow::anyhow;
use artifact::Artifact;
use backend::{postgres::PostgresBackend, sqlite::SqliteBackend};
use clap::Parser;
use log::{debug, info};
use manifest::{
    project::{open_project, Database, ProjectInfo},
    MANIFEST_FILENAME,
};
use migration::MigrationSet;
use std::{
    env,
    fs::File,
    io::{self, stdout},
    path::PathBuf,
};

pub mod actions;
mod arguments;
pub mod artifact;
mod backend;
pub mod manifest;
mod migration;
mod util;

pub const SQIGL_VERSION: &str = env!("CARGO_PKG_VERSION");

fn get_directory(directory: Option<PathBuf>) -> io::Result<PathBuf> {
    Ok(directory
        .map(Ok)
        .unwrap_or_else(env::current_dir)?
        .canonicalize()?
        .to_path_buf())
}

enum DatabaseBackend {
    Postgres(PostgresBackend),
    Sqlite(SqliteBackend),
}
impl DatabaseBackend {
    pub fn get(info: &ProjectInfo) -> anyhow::Result<Self> {
        match &info.database {
            Database::Postgres(params) => Ok(Self::Postgres(PostgresBackend::get(params)?)),
            Database::Sqlite(params) => {
                if let Some(path) = &params.path {
                    let db = {
                        if path.is_relative() {
                            rusqlite::Connection::open(info.root.join(path))?
                        } else {
                            rusqlite::Connection::open(path)?
                        }
                    };
                    Ok(Self::Sqlite(SqliteBackend::new(db)))
                } else {
                    let db = rusqlite::Connection::open_in_memory()?;
                    Ok(Self::Sqlite(SqliteBackend::new(db)))
                }
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = SqiglArguments::parse();
    simple_logger::SimpleLogger::new()
        .with_level(args.log_level.into())
        .init()
        .unwrap();

    debug!("sqigl Version: {}", SQIGL_VERSION);
    match args.command {
        Cmd::Project(cmd) => match cmd {
            ProjCmd::Init { title, database } => {
                let directory = env::current_dir()?;
                let manifest_path = directory.join(&title).with_file_name(MANIFEST_FILENAME);
                create_project(manifest_path, title, database)?;
            }
            ProjCmd::Create {
                title,
                directory,
                database,
            } => {
                let manifest_path = directory.join(&title).join(MANIFEST_FILENAME);
                create_project(manifest_path, title, database)?;
            }
            ProjCmd::Feature { title, project } => {
                let info = open_project(project.canonicalize()?)?;
                let new_version = new_feature(title, info)?;

                info!("Assigned preliminary version {}", new_version);
            }
            ProjCmd::Build {
                project,
                output,
                quiet,
            } => {
                let info = open_project(project.canonicalize()?)?;
                let artifact = build_project(&info)?;
                if let Some(output) = output {
                    if output.exists() {
                        return Err(anyhow!("Output already exists"));
                    }
                    let f = File::create_new(output)?;
                    let _id = artifact.write_to(f)?;
                } else if !quiet {
                    let _id = artifact.write_to(stdout())?;
                }
            }
            ProjCmd::Check { project } => {
                let info = open_project(project.canonicalize()?)?;
                let artifact = build_project(&info)?;
                match DatabaseBackend::get(&info)? {
                    DatabaseBackend::Postgres(backend) => check_artifact(artifact, backend)?,
                    DatabaseBackend::Sqlite(backend) => check_artifact(artifact, backend)?,
                };
            }
            ProjCmd::Apply { project } => {
                let info = open_project(project.canonicalize()?)?;
                let artifact = build_project(&info)?;
                match DatabaseBackend::get(&info)? {
                    DatabaseBackend::Postgres(backend) => apply_artifact(backend, artifact)?,
                    DatabaseBackend::Sqlite(backend) => apply_artifact(backend, artifact)?,
                };
            }
            ProjCmd::Save { project } => {
                let info = open_project(project.canonicalize()?)?;
                save_project(&info)?;
            }
            ProjCmd::Release { level, project } => {
                let info = open_project(project.canonicalize()?)?;
                let new_version = match DatabaseBackend::get(&info)? {
                    DatabaseBackend::Postgres(backend) => release(level, &info, backend)?,
                    DatabaseBackend::Sqlite(backend) => release(level, &info, backend)?,
                };
                info!("Released version {}", new_version);
            }
        },
        Cmd::Database(cmd) => match cmd {
            DbCmd::Install { project } => {
                let info = open_project(project)?;
                match DatabaseBackend::get(&info)? {
                    DatabaseBackend::Postgres(mut backend) => backend.install()?,
                    DatabaseBackend::Sqlite(mut backend) => backend.install()?,
                };
            }
            DbCmd::Apply { version, project } => {
                let info = open_project(project.canonicalize()?)?;
                match DatabaseBackend::get(&info)? {
                    DatabaseBackend::Postgres(backend) => apply_version(version, &info, backend)?,
                    DatabaseBackend::Sqlite(backend) => apply_version(version, &info, backend)?,
                };
            }
        },
        Cmd::Migration(cmd) => match cmd {
            MigrationCommands::Create { from, to, project } => {
                let info = open_project(project.canonicalize()?)?;
                create_migration(
                    from,
                    to.unwrap_or_else(|| info.project.version.clone()),
                    &info,
                )?;
            }
            MigrationCommands::Generate { from, to, project } => {
                let info = open_project(project.canonicalize()?)?;
                match DatabaseBackend::get(&info)? {
                    DatabaseBackend::Postgres(mut backend) => {
                        generate_migration(
                            from,
                            to.unwrap_or_else(|| info.project.version.clone()),
                            &mut backend,
                            &info,
                        )?;
                    }
                    DatabaseBackend::Sqlite(mut backend) => {
                        generate_migration(
                            from,
                            to.unwrap_or_else(|| info.project.version.clone()),
                            &mut backend,
                            &info,
                        )?;
                    }
                };
            }
        },
    }

    Ok(())
}
