-- Add migration script here
create table public.auto_replies
(
    user_id bigint                not null,
    keyword character varying(30) not null,
    count   bigint                not null,
    primary key (keyword, user_id)
);