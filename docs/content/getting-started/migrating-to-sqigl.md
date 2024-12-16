+++
title = "Migrating to `sqigl`"
weight = 2

[extra]
desc = "Migrate an existing schema to `sqigl`."
+++

# Create a new project

Create a new empty `sqigl` project.

```bash
sqigl project create my-project postgres
```

# Dump your schema

Dump the schema out of your database into a file in your project's `src/` directory.

{{ example(
postgres="```bash
pg_dump --schema-only > src/schema.sql
```",
sqlite="```bash
sqlite3 /path/to/database.db .schema > src/schema.sql
```"
) }}

# Release an initial version

```bash
sqigl project release
```

# Start a new feature

# Organize your code

# Release again
