-- Add migration script here
create table public.birthdays
(
    user_id            bigint not null primary key,
    guild_id           bigint not null,
    birthday           date   not null,
    last_congratulated date
);