create table public.auto_replies
(
    user_id bigint                not null,
    keyword character varying(30) not null,
    count   bigint                not null,
    primary key (keyword, user_id)
);

create table public.features
(
    id        serial,
    name      text                     not null,
    state     bigint                   not null,
    timestamp timestamp with time zone not null default now()
);

create table public.reminder
(
    channel_id bigint                   not null,
    msg_id     bigint primary key,
    user_id    bigint                   not null,
    time       timestamp with time zone not null,
    content    text                     not null
);

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
    primary key (emoji_id, message_id),
    foreign key (emoji_id) references public.unicode_to_emoji (id)
);
