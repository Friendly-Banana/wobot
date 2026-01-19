CREATE TABLE public.bets
(
    id          bigserial primary key,
    guild_id    bigint                   NOT NULL,
    channel_id  bigint                   NOT NULL,
    message_id  bigint                   NOT NULL,
    author_id   bigint                   NOT NULL,
    description text                     NOT NULL,
    expiry      timestamp with time zone NOT NULL,
    created_at  timestamp with time zone NOT NULL DEFAULT now()
);
CREATE INDEX ON public.bets (expiry);

CREATE TABLE public.bet_participants
(
    bet_id   bigint  NOT NULL REFERENCES public.bets (id) ON DELETE CASCADE,
    user_id  bigint  NOT NULL,
    watching boolean NOT NULL DEFAULT false,
    PRIMARY KEY (bet_id, user_id)
);
