+++
title = "Project organization"
weight = 0

[extra]
desc = "Modules, scripts, artifacts, and dependencies."
+++

# Projects

- Projects are a file tree containing the source code of your project.
- The top-level directory is called the project root.
- Below is an example of a project.

{{ filetree(path="filetree/simple.toml") }}

## Scripts

- Scripts are SQL files which contain the code to our project.
- Scripts must use the `.sql` extension.
- When we build our project, we will concatenate all of our scripts together into
    a single script.

## Modules

- Modules are directories which contain scripts.
- They are located in the `src/` directory.
    - The `src/` directory itself is a module.
- Modules may contain submodules, which are subdirectories containing their own scripts.
- This allows us to organize our code by topic, making our project more maintainable.
- Modules may contain a manifest.
    - The module manifest is a file called `sqigl.toml` which contains configuration
        information.

## Artifacts

- Artifacts are special modules containing [migration scripts](https://en.wikipedia.org/wiki/Schema_migration).
- They are automatically created by `sqigl` as needed. You won't need to create
    or manage them manually.
- Migrations move the database from one version to another.
- Unlike other modules, artifacts do not contain submodules.
- They are located in the `artifacts/` directory.
- They are named after the version they concern.

# Manifests

- Manifests are configuration files.
- They are always named `sqigl.toml`.
- Their function is determined by the directory they are in.

## Project manifest

- The project manifest specifies the project name, version, and database parameters.
- It's located in the project root.
- The project manifest is required.

```toml
[project]
version = "0.1.0"
title = "my_project"

[database]
db = "postgres"
```

## Module manifests

- Module manifests specify dependencies.
- They are optional. You won't need to use them most of the time.

```toml
[module]
dependencies = ["resources/"]

[[scripts]]
script = "posts.sql"
dependencies = ["users.sql"]
```

## Artifact manifests

- Artifacts are special modules containing scripts called migrations.
- Migrations move a database from one version to another.
- The module manifest specifies what versions are compatible with a migration,
    and what version they update the database to.
- Artifact manifests are required.

```toml
[[migrations]]
script = "schema.sql"
from = "=0.0.0" # The versions this migration is compatible with.
to = "0.1.0" # The version the migration moves the database to.
```

# Dependencies

- Sometimes SQL requires statements to appear in a certain order.
- The most common example is `create table` statements with a foreign-key relationship.
    ```sql
    -- Correct ✅
    create table users (
        pk integer primary key auto increment,
        username text unique not null,
        password text not null
    );
    create table posts (
        pk integer primary key auto increment,
        user integer not null primary key users(pk)
    );

    -- Incorrect ❌
    create table posts (
        pk integer primary key auto increment,
        user integer not null primary key users(pk)
    );
    create table users (
        pk integer primary key auto increment,
        username text unique not null,
        password text not null
    );
    ```
- When these statements are in different scripts, it creates a dependency relationship
    between them.
- If `sqigl` doesn't know about these dependencies, it won't be able to build our
    code correctly.

## Implicit dependencies

- Modules depend on their parent modules implicitly.
- Organizing our project so that our tables with references are at the leaves 
    of our project and tables that are reffered to are at the root naturally
    expresses our dependency relationships.
- Consider the following project:
    {{ filetree(path="filetree/implicit_deps.toml") }}
- Because `users.sql` is in the parent module of `posts.sql`, it will preceed
    `posts.sql` in the build.
    ```bash
    > sqigl project build
    -- [ my_project 0.1.0 ]

    -- [ users.sql ]

    create table users (
        pk integer primary key auto increment,
        username text unique not null,
        password text not null
    );

    -- [ resources/posts.sql ]

    create table posts (
        pk integer primary key auto increment,
        user integer not null primary key users(pk)
    );
    ```
- When possible, dependencies should be expressed implicitly.
- Otherwise, you will need to express the dependency explicitly in one of the following
    ways.

## Module-level dependencies

- We can add an entry to a module's manifest indicating that it depends on another module.
- This is useful when a module depends on a module which isn't an ancestor (a sibling for instance).
- Consider the following project:
    {{ filetree(path="filetree/module_deps.toml") }}
- Because the `posts/` module's manifest specifies the `users/` module as a dependency,
    `users.sql` will preceed `posts.sql` in the build.
    ```bash
    > sqigl project build
    -- [ my_project 0.1.0 ]

    -- [ users/users.sql ]

    create table users (
        pk integer primary key auto increment,
        username text unique not null,
        password text not null
    );

    -- [ posts/posts.sql ]

    create table posts (
        pk integer primary key auto increment,
        user integer not null primary key users(pk)
    );
    ```

## Script-level dependencies

- If we have dependencies between two scripts in the same module, we can specify
    this in the module's manifest.
- Consider the following project:
    {{ filetree(path="filetree/script_deps.toml") }}
- Because we've identified `users.sql` as a dependency of `posts.sql`, it will preceed
    `posts.sql` in the build.
    ```bash
    > sqigl project build
    -- [ my_project 0.1.0 ]

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

# Starting a new project

- To create a new, empty project, use the command `sqigl project create <project_name> <database>`.
- Database will be `postgres` or `sqlite`, depending on the database you're using.
- This will create a new directory named `<project_name>`, which will be more project root.

{{ example(
postgres="```bash
> sqigl project create my_project postgres
> cd my_project
> git init
Initialized empty Git repository in my_project
```",
sqlite="```bash
> sqigl project create my_project sqlite
> cd my_project
> git init
Initialized empty Git repository in my_project
```"
) }}

{{ filetree(path="filetree/empty_project.toml") }}
