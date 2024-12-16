+++
title = "Development environment"
weight = 1

[extra]
desc = "Setting up your local environment to work with `sqigl`."
+++

# Postgres 

## Setting up a local database

- If you are working with Postgres, you will need to set up a database locally.
- The easiest way to get started is using [Docker compose](https://docs.docker.com/compose/)
- You can use the following compose file.

```yaml
services:
  postgres:
    image: "postgres:15.2-bullseye"
    environment:
      POSTGRES_USER: "sqigl"
      POSTGRES_PASSWORD: "password"
      POSTGRES_DB: "sqigl"
    ports:
        - "5432:5432"
```

- Add the following line to your [`pgpass` file](https://www.postgresql.org/docs/current/libpq-pgpass.html)
    to access the database with `psql`.

```
localhost:5432:sqigl:sqigl:password
```

- If you want to configure Postgres yourself, make sure that `sqigl` has the
    ability to create & delete databases.

## Connecting to a production database

- Use a [`pgpass` file](https://www.postgresql.org/docs/current/libpq-pgpass.html)
    to supply credentials to `sqigl` when connecting to a production database.
- Add the other connection parameters to your project manifest.

```toml
[project]
version = "0.1.0"
title = "my_project"

[database]
db = "postgres"
database = "my_application"
hostname = "db.example.com"
```

# SQLite

- No setup is required to use `sqigl` with `sqlite`.
- `sqigl` will use in-memory databases when necessary.
