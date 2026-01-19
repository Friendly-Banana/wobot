CREATE TABLE public.birthdays
(
    user_id            bigint NOT NULL primary key,
    guild_id           bigint NOT NULL,
    birthday           date   NOT NULL,
    last_congratulated date
);