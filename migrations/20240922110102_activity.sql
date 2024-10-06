-- Add migration script here
create table public.activity
(
    guild_id    bigint not null,
    user_id     bigint not null,
    last_active date default now(),
    primary key (guild_id, user_id)
);