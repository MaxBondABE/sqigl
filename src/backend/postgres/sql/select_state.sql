select s.sqigl_version, h.version as project_version
from sqigl_internal.state as s left join sqigl_internal.history as h
on s.head = h.pk
