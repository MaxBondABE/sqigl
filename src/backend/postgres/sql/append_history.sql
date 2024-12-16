insert into sqigl_internal.history(prev, artifact, version)
values ($1, $2, $3)
returning pk
