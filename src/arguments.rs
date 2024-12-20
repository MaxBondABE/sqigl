use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use semver::{BuildMetadata, Prerelease, Version};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    backend::{self, postgres::PostgresBackend, sqlite::SqliteBackend, Backend},
    manifest,
};

#[derive(Parser, Clone, Debug, Serialize, Deserialize)]
pub struct SqiglArguments {
    /// Level at which to output logs to stderr
    #[arg(long, default_value = "info", env = "SQIGL_LOG_LEVEL")]
    pub log_level: LogLevel,
    #[command(subcommand)]
    pub command: SqiglCommands,
}

#[derive(ValueEnum, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum LogLevel {
    Off,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}
impl From<LogLevel> for log::LevelFilter {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Off => log::LevelFilter::Off,
            LogLevel::Trace => log::LevelFilter::Trace,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Error => log::LevelFilter::Error,
        }
    }
}

#[derive(Subcommand, Clone, Debug, Serialize, Deserialize)]
pub enum SqiglCommands {
    #[clap(subcommand)]
    Project(ProjectCommands),
    #[clap(subcommand)]
    Migration(MigrationCommands),
    #[clap(subcommand)]
    Database(DatabaseCommand),
}

#[derive(Subcommand, Clone, Debug, Serialize, Deserialize)]
pub enum ProjectCommands {
    /// Initialize a new project in the current directory.
    #[command()]
    Init {
        title: String,
        database: DatabaseKind,
    },

    /// Create a new sqigl project.
    #[command()]
    Create {
        title: String,
        database: DatabaseKind,
        /// The directory in which the new project's root directory will be created.
        #[arg(default_value = ".")]
        directory: PathBuf,
    },

    /// Begin working on a new feature. Assign a preliminary version number, including
    /// prerelease information.
    #[command()]
    Feature {
        /// An ID for the change, such as a ticket number. Must be alphanumeric
        /// with hyphens, matching `[0-9A-Za-z-]+`.
        title: String,
        /// The project root (or any of it's subdirectories).
        #[arg(default_value = ".")]
        project: PathBuf,
    },

    /// Build a sqigl project, and output it's contents.
    #[command()]
    Build {
        #[arg(default_value = ".")]
        project: PathBuf,
        /// Write output to a file instead of printing it to stdout.
        output: Option<PathBuf>,
        /// Do not print the build to stdout.
        #[arg(short, long)]
        quiet: bool,
    },

    /// Build & apply the current version of the project to an empty database
    /// to check for errors, before rolling back the changes.
    #[command()]
    Check {
        #[arg(default_value = ".")]
        project: PathBuf,
    },

    /// Apply the current state of the project to the database. This is for
    /// development use; use the `database` subcommand for production.
    #[command()]
    Apply {
        #[arg(default_value = ".")]
        project: PathBuf,
    },

    /// Build the current version of the project and save it as a migration.
    #[command()]
    Save {
        #[arg(default_value = ".")]
        project: PathBuf,
    },

    /// Assign a project a release number & save it under it's new version.
    #[command()]
    Release {
        #[arg(default_value = "feature")]
        level: ReleaseLevel,
        #[arg(default_value = ".")]
        project: PathBuf,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DatabaseKind {
    Postgres,
    Sqlite,
}
impl From<DatabaseKind> for manifest::project::Database {
    fn from(value: DatabaseKind) -> Self {
        match value {
            DatabaseKind::Postgres => manifest::project::Database::Postgres(Default::default()),
            DatabaseKind::Sqlite => manifest::project::Database::Sqlite(Default::default()),
        }
    }
}

#[derive(Subcommand, Clone, Debug, Serialize, Deserialize)]
pub enum DatabaseCommand {
    /// Install `sqigl` onto the database.
    Install {
        #[arg(default_value = ".")]
        project: PathBuf,
    },

    /// Apply the appropriate migration to update the database to the supplied
    /// version.
    Apply {
        version: Version,
        #[arg(default_value = ".")]
        project: PathBuf,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ReleaseLevel {
    Patch,
    Minor,
    Major,
}
impl ReleaseLevel {
    pub fn release_version(&self, latest: &Version) -> Version {
        let (major, minor, patch) = match self {
            ReleaseLevel::Patch => (latest.major, latest.minor, latest.patch + 1),
            ReleaseLevel::Minor => (latest.major, latest.minor + 1, 0),
            ReleaseLevel::Major => (latest.major + 1, 0, 0),
        };

        Version {
            major,
            minor,
            patch,
            build: BuildMetadata::EMPTY,
            pre: Prerelease::EMPTY,
        }
    }
}

#[derive(Subcommand, Clone, Debug, Serialize, Deserialize)]
pub enum MigrationCommands {
    /// Create a new, empty migration.
    Create {
        /// The version to migrate from.
        from: Version,
        /// The version to migrate to.
        to: Option<Version>,
        /// The directory in which the new project's root directory will be created.
        #[arg(default_value = ".")]
        project: PathBuf,
    },

    /// Generate a new migration.
    Generate {
        /// The version to migrate from.
        from: Version,
        /// The version to migrate to.
        to: Option<Version>,
        /// The directory in which the new project's root directory will be created.
        #[arg(default_value = ".")]
        project: PathBuf,
    },

    /// Run a migration against an empty database, and roll it back
    Check {
    },

    /// Create a new database, and apply the migrations
    Apply {
    }
}
