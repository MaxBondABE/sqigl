+++
title = "Building"
weight = 1

[extra]
desc = "Assembling scripts into an executable artifact."
+++

# Building a project

- Building is the process of combining all of the code in the project into a
    single executable artifact.
    - This artifact can be run on a database to create your schema
- When `sqigl` builds a project, it walks the `src/` directory and identifies all
    scripts within the project
- The scripts are [sorted](https://en.wikipedia.org/wiki/Topological_sorting) based
    on their dependency relationships.
- They are then concatenated together in order into a single script.

{{ filetree(path="filetree/unsaved.toml") }}

```bash
> sqigl project build
-- [ my_project 0.2.0-my-feature ]

-- [ users.sql ]

create table users (
    pk integer primary key auto increment,
    username text unique not null,
    password text not null
);

-- [ posts.sql ]

create table posts (
    pk integer primary key auto increment,
    user integer not null primary key users(pk)
);
```

# Dependency cycles

- The dependency relationships of a project must form a [DAG.](https://en.wikipedia.org/wiki/Directed_acyclic_graph)
- This means there must be no dependency cycles. A dependency cycle will cause a build to fail.
- A cycle is formed when a module's dependency depends on that module.
    - Note that this may be a transitive dependency, eg the cycle may be between many modules.
- The following project contains a cycle:
    {{ filetree(path="filetree/cycle.toml") }}
- Building the project results in an error.

```bash
> sqigl project build
Error: A cycle exists between foo.sql and baz.sql:
    foo.sql -->
    bar.sql -->
    baz.sql --> foo.sql
```

# Saving a build

- `sqigl project build` will always build the current revision of your project,
    including any changes you've made but have not saved
- To save these changes, use `sqigl project save`.
- This will create a script called `schema.sql` in the artifact module corresponding
    to the version configured in the project manifest.
    - If this file exists, it will be overwritten.

```bash
> sqigl project save
2025-01-01T00:00:00.000Z INFO  [sqigl::actions::save] Saving project
```

{{ filetree(path="filetree/saved.toml") }}
