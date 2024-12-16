+++
title = "Versioning"
weight = 4

[extra]
desc = "Assigning version numbers to in-progress and released revisions."
+++

# Empty databases

- The version `0.0.0` is reserved for empty databases (databases to which no migrations
    have been applied).
- When you build your project, the artifact produced is a migration from `0.0.0`
    to the project's current version.

# Feature versions

- Feature versions are prelimiary versions assigned to new work on a `sqigl` project.
- Any version with prerelease information is considered a feature version.
    - For example `1.2.3-foo`
- A feature version is assigned with the `sqigl project feature <id>` command.
    - The `<id>` value should be a ticket number or branch name that identifies the work
    - It must be alphanumeric with hyphens, matching the regex `[0-9A-Za-z-]+`.
- The feature version will be the next availble minor version, with the `<id>` value
    populating the prerelease field of the semantic version.
    - Eg, if the project is at version `1.2.3` and you run `sqigl project feature my-feature`,
    the feature version will be `1.3.0-my-feature`.
    - The project manifest is then updated to reflect this version.
- A final version number will be assigned when the feature is released.

```bash
> git checkout -b my-feature
Switched to a new branch 'my-feature'
> sqigl project feature my-feature
2025-01-01T00:00:00.000Z INFO  [sqigl] Assigned preliminary version 0.2.0-my-feature
```

# Release versions

- The released version represents a finalized version which is ready to be applied
    to a production database.
- Any version without prerelease information is considered a release version.
    - For example `1.2.3`
- It is assigned by running `sqigl project release <level>` from a project on a feature
    version.
    - The `<level>` value is either `patch`, `minor`, or `major`, and reflects the 
        semantic version bump which the change requires based on its
        [compatability](@/concepts/compatability.md).
- `sqigl` look at the latest version number in it's `artifacts/` directory, and the
    latest version available in the database.
- It will assign the next available version at the required release level, and strip
    away the prerelease information.
- Any migrations you created under the feature version are updated to reflect the
    new version.
- After a release, `sqigl` always saves the current build.
