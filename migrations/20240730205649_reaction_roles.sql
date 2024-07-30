-- Add migration script here
create table public.unicode_to_emoji
(
    id      bigserial primary key,
    unicode text not null unique
);

create table public.reaction_roles
(
    message_id bigint not null,
    channel_id bigint not null,
    guild_id   bigint not null,
    role_id    bigint not null,
    emoji_id   bigint not null,
    primary key (emoji_id, message_id)
);