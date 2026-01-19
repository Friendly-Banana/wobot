CREATE TABLE public.modules
(
    guild_id  bigint NOT NULL,
    module_id int    NOT NULL,
    primary key (guild_id, module_id)
);