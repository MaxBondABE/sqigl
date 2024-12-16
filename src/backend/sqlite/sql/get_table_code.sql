select sql from sqlite_schema
where type = 'table' and tbl_name = ?1
