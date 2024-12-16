select table_schema, table_name
from information_schema.tables 
where table_schema not in ('information_schema', 'pg_catalog', 'pg_toast');
