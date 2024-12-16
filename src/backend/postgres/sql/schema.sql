create schema if not exists sqigl_internal;

-- Built artifacts which have been applied to the database
create table if not exists sqigl_internal.artifacts (
    pk bigint primary key generated always as identity,
    id bytea not null, -- SHA256(content)
    created_at timestamptz not null default now(),
    updated_at timestamptz,
    content text
);

-- Performs better than unique/btree index on uniformly random data
create index if not exists id_idx on sqigl_internal.artifacts using hash (id);

-- Tree/persistent list of operations applied to this database
create table if not exists sqigl_internal.history (
    pk bigint primary key generated always as identity,
    prev bigint references sqigl_internal.history(pk), -- head before we applied change
    artifact bigint not null references sqigl_internal.artifacts(pk),
    created_at timestamptz not null default now(),
    updated_at timestamptz,
    version text not null, -- semver of the sqigl project after artifact applied
    remarks text
);

-- Current database state
create table if not exists sqigl_internal.state (
    -- Ensure there is at most 1 row
    pk integer primary key generated always as (0) stored,
    created_at timestamptz not null default now(),
    updated_at timestamptz,
    head bigint references sqigl_internal.history(pk), -- last change applied
    sqigl_version text not null -- semver of the sqigl binary which installed db
);
