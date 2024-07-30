-- Add migration script here
create table public.reminder
(
    channel_id bigint                   not null,
    msg_id     bigint primary key,
    user_id    bigint                   not null,
    time       timestamp with time zone not null,
    content    text                     not null
);