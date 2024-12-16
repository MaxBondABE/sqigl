sqigl - A build tool for SQL
----------------------------

# Experimental software 🧪

This is an experimental version of `sqigl` distributed to solicit feedback. It is
not ready for production use.

# sqigl ("squiggle") lets you write SQL fluently

- Traditional schema management tools force you to write your code as a linear
    series of migrations.
- This is cumbersome when you are taking full advantage of SQL's features, such as
    stored procedures
- sqigl allows you to organize your SQL like you would any other codebase - 
    as a file tree organized by topic.

# Documentation

- [Here](https://maxbondabe.github.io/)
- [Quickstart](https://maxbondabe.github.io/getting-started/quickstart)

# Installation

TBD

# Known issues

- `sqigl` assumes that all schema changes are managed from within `sqigl`
    - If you manually change the database, or apply migrations using a different tool,
        then `sqigl`'s history tables will not reflect these changes & it won't
        be able to detect any incompatabilities that have been introduced
    - eg, if you manually connect to the database and change the name of a table,
        `sqigl` won't be detect any incompatabilities this has introduced in your
        code managed by `sqigl`
    - A repair tool will be included in a future version to address issues like this
