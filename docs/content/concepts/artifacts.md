+++
title = "Artifacts and migrations"
weight = 2

[extra]
desc = "Managing builds and database schema."
+++

# Artifacts

- Artifacts are special modules containing [migration scripts](https://en.wikipedia.org/wiki/Schema_migration).
- They are automatically created by `sqigl` as needed. You won't need to create
    or manage them manually.
- Unlike other modules, artifacts do not contain submodules.
- They are located in the `artifacts/` directory.
- They are named after the version they concern.

## Migrations

- Migrations move the database from one version to another.
- Migrations are configured in the artifact manifest.
- They have 3 parameters:
    - `script` - the filename of the migration script.
    - `from` - the semantic versioning requirement specifying which versions are compatible with the migration.
    - `to` - the version of the database after the migration has been applied.

{{ filetree(path="filetree/simple2.toml") }}

# Migrations from `0.0.0`

- When you save a build, it is saved as a migration named `schema.sql`.
- This represents a migration from `0.0.0` to the current version.

# Generating migrations

- `sqigl` can automatically generate some types of migration.
- Currently, only creating and dropping tables are supported.
    - More types of migration will be supported in the future.
- This works by creating a database and briging it up to each version, and
    looking for differences in the schema.
- To generate a migration, use the command `sqigl migration generate <from> (to)`
    - If `to` is not specified, the current project version is used.

# Applying migrations

- Migrations are applied with the `sqigl database applied <version>` command;
