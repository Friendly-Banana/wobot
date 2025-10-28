-- Add migration script here
alter table public.activity
    add column message_count bigint not null default 0;
