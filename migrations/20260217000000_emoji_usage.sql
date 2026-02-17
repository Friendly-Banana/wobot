CREATE TABLE public.emoji_usage
(
    guild_id bigint NOT NULL,
    emoji_id bigint NOT NULL,
    count    bigint NOT NULL DEFAULT 1,
    PRIMARY KEY (guild_id, emoji_id)
);

