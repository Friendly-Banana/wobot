-- Add migration script here
create table public.features
(
    id        serial,
    name      text                     not null,
    state     bigint                   not null,
    timestamp timestamp with time zone not null default now()
);