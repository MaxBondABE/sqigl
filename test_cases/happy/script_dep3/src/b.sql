create table bar(
    a integer primary key,
    b integer references foo(a)
);
