create table if not exists sqigl_internal_artifacts (
    pk integer primary key autoincrement,
    id blob unique not null, -- SHA256(content)
    created_at integer not null default (unixepoch()),
    updated_at integer,
    content text
) strict;

create table if not exists sqigl_internal_history (
    pk integer primary key autoincrement,
    prev integer references sqigl_internal_history(pk),
    artifact integer not null references sqigl_internal_artifacts(pk),
    created_at integer not null default (unixepoch()),
    updated_at integer,
    version text not null, -- semver
    remarks text,

    -- pk = 1 iif prev is not null
    check(pk != 1 or prev is null),
    check(pk = 1 or prev is not null)
) strict;

create table if not exists sqigl_internal_state (
    -- Ensure there is at most 1 row
    pk integer primary key default 0 check (pk = 0),
    created_at integer not null default (unixepoch()),
    updated_at integer,
    head integer references sqigl_internal_history(pk),
    sqigl_version text not null -- semver
) strict;
