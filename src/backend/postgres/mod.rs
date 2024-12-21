mod delta;

use std::{
    env::{self, VarError},
    fs,
    num::NonZeroU16,
    path::PathBuf,
};

use crate::{
    artifact::{Artifact, ConsumerError, ContentId, ScriptConsumer, ScriptProcessingError},
    manifest::{self, project::PostgresDatabase},
    util::{empty_database_version, from_minor_version},
    SQIGL_VERSION,
};
use anyhow::anyhow;
use log::{debug, error, info, trace, warn};
use native_tls::{Certificate, TlsConnector};
use postgres::Client;
use postgres_native_tls::MakeTlsConnector;
use postgres_secrets::{
    pgpass::{CredentialQuery, LoadError},
    PgPass,
};
use semver::Version;

use self::delta::delta;

use super::{Backend, GeneratedMigration, SqiglState};

// Unofficial
pub const STATEMENT_TIMEOUT_ENVVAR: &str = "PGSTATEMENT_TIMEOUT";
pub const TRANSACTION_TIMEOUT_ENVVAR: &str = "PGTRANSACTION_TIMEOUT";

// https://www.postgresql.org/docs/current/libpq-envars.html
pub const HOSTNAME_ENVVAR: &str = "PGHOST";
pub const PORT_ENVVAR: &str = "PGPORT";
pub const DATABASE_ENVVAR: &str = "PGDATABASE";
pub const USERNAME_ENVVAR: &str = "PGUSER";
pub const PASSWORD_ENVVAR: &str = "PGPASSWORD";
pub const CERTIFICATE_ENVVAR: &str = "PGROOTCERT";

impl ConsumerError for postgres::Error {}

fn get_state<Db: postgres::GenericClient>(db: &mut Db) -> Result<SqiglState, postgres::Error> {
    Ok(db
        .query_one(include_str!("sql/select_state.sql"), &[])?
        .try_into()?)
}

fn get_envvar(var: &str) -> anyhow::Result<Option<String>> {
    match env::var(var) {
        Ok(x) => Ok(Some(x)),
        Err(VarError::NotPresent) => Ok(None),
        Err(e) => Err(anyhow!(
            "Failed to load environment variable {}: {}",
            var,
            e
        )),
    }
}

fn get_port_envvar() -> anyhow::Result<Option<NonZeroU16>> {
    if let Some(digits) = get_envvar(PORT_ENVVAR)? {
        let port: u16 = match digits.parse() {
            Ok(x) => x,
            Err(e) => return Err(anyhow!("Could not parse port environment variable: {}", e)),
        };
        if let Some(port) = NonZeroU16::new(port) {
            Ok(Some(port))
        } else {
            Err(anyhow!(
                "Could not parse port environment variable: 0 is not a valid port number."
            ))
        }
    } else {
        Ok(None)
    }
}

fn get_timeout_envvar(envvar: &str) -> anyhow::Result<Option<f32>> {
    if let Some(digits) = get_envvar(envvar)? {
        let value: f32 = match digits.parse() {
            Ok(x) => x,
            Err(e) => return Err(anyhow!("Could not parse {}: {}", envvar, e)),
        };
        if value >= 0. {
            Ok(Some(value))
        } else {
            Err(anyhow!("Could not parse {}: Must be >= 0", envvar,))
        }
    } else {
        Ok(None)
    }
}

pub struct PostgresBackend {
    tls: MakeTlsConnector,
    config: postgres::Config,
    db: Client,

    // Timeouts are in milliseconds, because a timeout w/o a unit is interpreted
    // as milliseconds
    // https://www.postgresql.org/docs/17/runtime-config-client.html#GUC-STATEMENT-TIMEOUT
    stmt_timeout: Option<usize>,
    tx_timeout: Option<usize>,
}
impl PostgresBackend {
    pub fn new(
        config: postgres::Config,
        stmt_timeout: Option<usize>,
        tx_timeout: Option<usize>,
    ) -> Result<Self, postgres::Error> {
        let tls = MakeTlsConnector::new(TlsConnector::new().unwrap());
        let db = config.clone().connect(tls.clone())?;
        Ok(Self {
            db,
            config,
            tls,
            stmt_timeout,
            tx_timeout,
        })
    }
    pub fn new_tls(
        config: postgres::Config,
        tls: MakeTlsConnector,
        stmt_timeout: Option<usize>,
        tx_timeout: Option<usize>,
    ) -> Result<Self, postgres::Error> {
        let db = config.clone().connect(tls.clone())?;
        Ok(Self {
            db,
            config,
            tls,
            stmt_timeout,
            tx_timeout,
        })
    }
    pub fn local() -> Result<Self, postgres::Error> {
        let tls = MakeTlsConnector::new(TlsConnector::new().unwrap());
        let mut config = postgres::Client::configure();
        config
            .user("sqigl")
            .password("password")
            .host("localhost")
            .dbname("sqigl");
        let db = config.clone().connect(tls.clone())?;
        Ok(Self {
            db,
            config,
            tls,
            stmt_timeout: Default::default(),
            tx_timeout: Default::default(),
        })
    }
    pub fn get(params: &manifest::project::PostgresDatabase) -> anyhow::Result<Self> {
        let tls = {
            if let Some(path) = get_envvar(CERTIFICATE_ENVVAR)?
                .map(PathBuf::from)
                .as_ref()
                .or(params.certificate.as_ref())
            {
                let content = match fs::read(path.as_path()) {
                    Ok(x) => x,
                    Err(e) => {
                        return Err(anyhow!("Failed to read certificate: {}", e));
                    }
                };
                let cert = Certificate::from_pem(&content)?;
                let connector = TlsConnector::builder().add_root_certificate(cert).build()?;
                MakeTlsConnector::new(connector)
            } else {
                let connector = TlsConnector::new()?;
                MakeTlsConnector::new(connector)
            }
        };
        let hostname = get_envvar(HOSTNAME_ENVVAR)?.or_else(|| params.hostname.clone());
        let port = get_port_envvar()?.or(params.port);
        let database = get_envvar(DATABASE_ENVVAR)?.or_else(|| params.database.clone());
        let username = get_envvar(USERNAME_ENVVAR)?.or_else(|| params.username.clone());
        let password = get_envvar(PASSWORD_ENVVAR)?;

        // Timeouts are specified as f32 of seconds but stored as milliseconds because
        // a timeout w/o a unit is interpreted as milliseconds
        // https://www.postgresql.org/docs/17/runtime-config-client.html#GUC-STATEMENT-TIMEOUT
        let stmt_timeout: Option<usize> = get_timeout_envvar(STATEMENT_TIMEOUT_ENVVAR)?
            .or(params.statement_timeout)
            .map(|t| (t * 1000.) as usize);
        let tx_timeout: Option<usize> = get_timeout_envvar(TRANSACTION_TIMEOUT_ENVVAR)?
            .or(params.transaction_timeout)
            .map(|t| (t * 1000.) as usize);

        if password.is_some() {
            let Some(hostname) = hostname else {
                return Err(anyhow!(
                    "Could not connect to database: Hostname was not supplied."
                ));
            };
            let Some(port) = port else {
                return Err(anyhow!(
                    "Could not connect to database: Port was not supplied."
                ));
            };
            let Some(database) = database else {
                return Err(anyhow!(
                    "Could not connect to database: Database was not supplied."
                ));
            };
            let Some(username) = username else {
                return Err(anyhow!(
                    "Could not connect to database: Username was not supplied."
                ));
            };

            let mut config = postgres::Config::new();
            config
                .host(&hostname)
                .port(port.get())
                .dbname(&database)
                .user(&username);
            return Ok(Self::new_tls(config, tls, stmt_timeout, tx_timeout)?);
        }

        let pgpass = match PgPass::load() {
            Ok(x) => x,
            Err(LoadError::CouldNotLocate) => {
                return Err(anyhow!(
                    "Could not connect to database: Credentials were not supplied."
                ));
            }
            Err(e) => {
                return Err(anyhow!("Failed to load pgpass file: {}", e));
            }
        };
        let query = CredentialQuery {
            hostname,
            port,
            database,
            username,
        };
        if let Some(creds) = pgpass.find(&query)? {
            Ok(Self::new_tls(creds.into(), tls, stmt_timeout, tx_timeout)?)
        } else {
            Err(anyhow!(
                "Could not connect to database: Credentials were not found in pgpass file."
            ))
        }
    }
    /// Open transaction & sets statement and transaction timeouts.
    fn open_transaction(&mut self) -> Result<postgres::Transaction, postgres::Error> {
        let mut tx = self.db.transaction()?;

        if let Some(timeout) = self.stmt_timeout {
            debug!("Setting statement timeout to {:.2}s", timeout as f32 / 60.);
            tx.execute(&format!("set local statement_timeout = {}", timeout), &[])?;
        }
        if let Some(timeout) = self.tx_timeout {
            debug!(
                "Setting transaction timeout to {:.2}s",
                timeout as f32 / 60.
            );
            match tx.execute(&format!("set local transaction_timeout = {}", timeout), &[]) {
                Ok(_) => (),
                Err(e) => {
                    if tx
                        .query_one(include_str!("sql/supports_transaction_timeout.sql"), &[])
                        .map(|row| row.get::<_, bool>(0))
                        .unwrap_or(false)
                    {
                        // We ignore errors here, because this is a best-effort attempt to
                        // provide additional context. The first error is considered canonical.
                        error!(
                            "transaction_timeout was specified, but this database doesn't appear \
                            to support it. This parameter was added in Postgres 17."
                        )
                    };

                    return Err(e);
                }
            }
        }

        Ok(tx)
    }
}
impl Backend for PostgresBackend {
    type Error = postgres::Error;

    fn install(&mut self) -> Result<SqiglState, Self::Error> {
        info!("Installing sqigl onto database.");
        let mut tx = self.db.transaction()?;
        tx.batch_execute(include_str!("sql/schema.sql"))?;
        tx.execute(include_str!("sql/initialize_state.sql"), &[&SQIGL_VERSION])?;
        let state = get_state(&mut tx)?;
        tx.commit()?;
        Ok(state)
    }
    fn open(&mut self) -> Result<SqiglState, Self::Error> {
        info!("Opening database.");
        let state = {
            if let Ok(state) = get_state(&mut self.db) {
                state
            } else {
                warn!("sqigl is not installed on this database; installing");
                self.install()?
            }
        };

        debug!(
            "Project Version: {} DB sqigl Version: {}",
            &state.project_version, &state.sqigl_version
        );
        Ok(state)
    }

    fn apply<A: Artifact>(
        &mut self,
        artifact: &A,
    ) -> Result<SqiglState, ScriptProcessingError<Self::Error>> {
        info!("Applying artifact.");
        struct Consumer<'a> {
            version: &'a Version,
            tx: postgres::Transaction<'a>,
        }
        impl ScriptConsumer for Consumer<'_> {
            type Error = postgres::Error;

            fn accept(&mut self, script: &str) -> Result<(), ScriptProcessingError<Self::Error>> {
                trace!("Running a script.");
                self.tx.batch_execute(script)?;
                Ok(())
            }

            fn commit(mut self, id: ContentId) -> Result<(), ScriptProcessingError<Self::Error>> {
                // NB: An artifact may be applied multiple times. Our artifact's row may
                // already exist.
                trace!("Committing artifact.");
                let artifact_pk: i64 = self
                    .tx
                    .query_one(
                        include_str!("sql/get_artifact_by_id.sql"),
                        &[&&id.unwrap().as_slice()],
                    )?
                    .get("pk");
                let prev_pk: Option<i64> = self
                    .tx
                    .query_one("select head from sqigl_internal.state", &[])?
                    .get("head");
                let head_pk: i64 = self
                    .tx
                    .query_one(
                        include_str!("sql/append_history.sql"),
                        &[&prev_pk, &artifact_pk, &self.version.to_string()],
                    )?
                    .get("pk");
                let updated = self
                    .tx
                    .execute("update sqigl_internal.state set head = $1", &[&head_pk])?;
                debug_assert!(updated == 1);

                self.tx.commit()?;
                debug!("Artifact transaction committed.");
                Ok(())
            }
        }

        // We must ensure:
        // - All migrations are atomic
        // - Migrations are only applied to compatible versions
        // To do this, we ensure:
        // - All sqigl instances run serially
        // - All statements are executed in a single transaction
        // - The project version is compatible at the start of the transaction
        debug!("Opening artifact transaction.");
        let mut tx = self.open_transaction()?; // Sets timeouts
        tx.execute("select from sqigl_internal.state for update", &[])?;
        let state = get_state(&mut tx)?;
        if !artifact.compatible(&state.project_version) {
            error!("Migration aborted: Incompatible");
            return Err(ScriptProcessingError::Incompatible);
        }

        let version = artifact.version();
        let consumer = Consumer { version, tx };
        artifact.scripts(consumer)?;

        info!("Migration applied.");
        Ok(state)
    }

    fn generate_migration<A1: Artifact, A2: Artifact>(
        &mut self,
        from: &A1,
        to: &A2,
    ) -> anyhow::Result<impl Artifact> {
        assert!(from.compatible(&empty_database_version()));
        assert!(to.compatible(&empty_database_version()));

        let from_db_name = format!("sqigl_tmp_{}", rand::random::<u32>());
        let to_db_name = format!("sqigl_tmp_{}", rand::random::<u32>());
        for name in [&from_db_name, &to_db_name] {
            self.db.execute(&format!("create database {}", name), &[])?;
        }

        let mut from_db = self
            .config
            .clone()
            .dbname(&from_db_name)
            .connect(self.tls.clone())?;
        from_db.batch_execute(&from.to_string())?;
        let mut to_db = self
            .config
            .clone()
            .dbname(&to_db_name)
            .connect(self.tls.clone())?;
        to_db.batch_execute(&to.to_string())?;

        let statements = delta(from_db, to_db)?;
        for name in [&from_db_name, &to_db_name] {
            self.db.execute(&format!("drop database {}", name), &[])?;
        }

        Ok(GeneratedMigration {
            from: from_minor_version(from.version()),
            to: to.version().clone(),
            statements,
        })
    }

    fn check<A: Artifact>(
        &mut self,
        artifact: &A,
    ) -> Result<(), ScriptProcessingError<Self::Error>> {
        info!("Checking artifact.");

        let _ = self.open()?;
        struct Consumer<'a> {
            tx: postgres::Transaction<'a>,
        }
        impl ScriptConsumer for Consumer<'_> {
            type Error = postgres::Error;

            fn accept(&mut self, script: &str) -> Result<(), ScriptProcessingError<Self::Error>> {
                trace!("Running a script.");
                self.tx.batch_execute(script)?;
                Ok(())
            }

            fn commit(mut self, id: ContentId) -> Result<(), ScriptProcessingError<Self::Error>> {
                trace!("Done checking, rolling back.");
                self.tx.rollback()?;
                Ok(())
            }
        }

        let mut tx = self.open_transaction()?; // Sets timeouts
        let state = get_state(&mut tx)?;
        if !artifact.compatible(&state.project_version) {
            error!("Migration aborted: Incompatible");
            return Err(ScriptProcessingError::Incompatible);
        }

        let consumer = Consumer { tx };
        artifact.scripts(consumer)?;

        Ok(())
    }
}
impl Default for PostgresBackend {
    fn default() -> Self {
        Self::local().unwrap()
    }
}

impl TryFrom<postgres::Row> for SqiglState {
    type Error = postgres::Error;

    fn try_from(row: postgres::Row) -> Result<Self, Self::Error> {
        Ok(Self {
            project_version: row
                .try_get::<'_, _, Option<String>>("project_version")?
                .map(|v| {
                    v.parse()
                        .expect("Failed to parse semver in project_version")
                })
                .unwrap_or_else(empty_database_version),
            sqigl_version: row
                .try_get::<'_, _, String>("sqigl_version")?
                .parse()
                .expect("Failed to parse semver in sqigl_version"),
        })
    }
}
