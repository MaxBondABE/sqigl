+++
title = "Quickstart"
weight = 0
draft = true

[extra]
desc = "A brief introduction to `sqigl`'s principles and workflow."
+++

# What is `sqigl`?

- `sqigl` (pronounced "squiggle") is a build tool for SQL
- It supports Postgres and SQLite
- Unlike traditional schema management tools, `sqigl` allows you to organize your SQL
    code in a file tree organized by topic
- When you build a `sqigl` project, your scripts are concatenated together into one
    SQL script which can then be run on your database

# Modules and Scripts

- *Modules* are directories containing SQL files called *scripts*
- 

*Further reading:* [Project organization](@/concepts/project.md)

# Manifests

- *Manifests* are TOML files with the name `sqigl.toml`
- Manifests configure the behavior of `sqigl`

# Building your project

*Further reading:* [Building](@/concepts/build.md)

# Artifacts

*Further reading:* [Schema management](@/concepts/artifacts.md)

# Workflow

## Create a new project

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

Your project will look like this.

{{ filetree(path="filetree/empty_project.toml") }}

## Start a new feature

```bash
> git checkout -b my-feature
Switched to a new branch 'my-feature'
> sqigl project feature my-feature
2025-01-01T00:00:00.000Z INFO  [sqigl] Assigned preliminary version 0.2.0-my-feature
```

{{ filetree(path="filetree/empty_project_with_version.toml") }}

## Edit your files

## Create a release

## Apply your changes
