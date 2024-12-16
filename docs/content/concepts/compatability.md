+++
title = "Compatability"
weight = 3

[extra]
desc = "Applying API compatability to SQL."
+++

# Semantic versioning

- `sqigl` uses [semantic versioning](https://devhints.io/semver) to to determine
which migrations may be safely applied to a database.
- Semantic versions take the form `<major>.<minor>.<patch>`, such as `1.2.3`
- A patch version bump indicates a change introduces a bug fix which does not
    impact compatability, such as inserting or updating a row
- A minor version bump indicates a backwards compatible change
    - A backwards compatible change will not disrupt any code currently consuming
        the database
- A major version bump indicates backwards a backwards incompatible change
    - This change is expected to interupt

# Compatability

- Compatability is central to semantic versioning.
- In order to determine the compatibility of a database migration, you need to
    consider it's visibility to database clients.
    - This is a perspective which may be unfamiliar to database programmers.
        It requires considering your schema as an API which applications consume.
- Below is an incomplete account of how SQL operations relate to semantic versioning
    compatability.
    - Because this is impacted by the your specific use case, it is not possible to
        provide universal guidance. You must interpret this in the context of your
        own application.
    - When in doubt, consider the changes backwards incompatible.

{{ compat() }}

# Managing compatibility

## Scream testing

- Before performing an irrevocable schema change, it is best practice to first perform a 
    ["scream test"](https://www.microsoft.com/insidetrack/blog/microsoft-uses-a-scream-test-to-silence-its-unused-servers/)
- A scream test is a change which will break the same code, but which is reversible
    - The goal is that to alert the impacted users and cause them to reach out to you
    - For example, if we wish to drop a table, our scream test will be to rename the table instead
- In complex deployments, there may be reporting which runs infrequently, such as on a monthly, quarterly, or annual basis
- A scream test should last at least as long as your organization's least-frequent reporting interval

## Decoupling with views

- In applications with complex compatibility requirements, views and stored procedures can be used to decouple 
    access to the schema from the underlying tables.
- For instance, you could create a view `users_v1` which applications use to access your `users` table.
    - Any backwards-incompatible changes to the `users` table will be paired with
        an update to the `users_v1` view to obscrue the change.
    - Then you will create a new view, `users_v2`, which reflects the change.
    - Access to the `users_v1` view will be deprecated and slowly phased out.
- Similarly, you can mandate that all access to your schema is performed through stored procedures, 
- See also [this article](https://fly.io/blog/skip-the-api/) about using database
    schema as an API, and the [Hacker News thread](https://news.ycombinator.com/item?id=37497345)
    for discussion about how to manage the resulting complications.
