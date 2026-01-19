CREATE TABLE public.unicode_to_emoji
(
    id      bigserial primary key,
    unicode text NOT NULL unique
);

CREATE TABLE public.reaction_roles
(
    message_id bigint NOT NULL,
    channel_id bigint NOT NULL,
    guild_id   bigint NOT NULL,
    role_id    bigint NOT NULL,
    emoji_id   bigint NOT NULL,
    primary key (emoji_id, message_id)
);