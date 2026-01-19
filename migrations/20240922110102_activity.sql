CREATE TABLE public.activity
(
    guild_id    bigint NOT NULL,
    user_id     bigint NOT NULL,
    last_active date DEFAULT now(),
    primary key (guild_id, user_id)
);