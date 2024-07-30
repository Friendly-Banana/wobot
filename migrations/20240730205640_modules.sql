-- Add migration script here
create table public.modules
(
    guild_id  bigint not null,
    module_id int    not null,
    primary key (guild_id, module_id)
);