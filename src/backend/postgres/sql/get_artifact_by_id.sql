insert into sqigl_internal.artifacts(id) values ($1)
on conflict do nothing
returning pk
