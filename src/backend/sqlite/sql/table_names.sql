select tbl_name from sqlite_schema
where
    type = 'table' and
    tbl_name not like 'sqlite_%'
group by tbl_name;
