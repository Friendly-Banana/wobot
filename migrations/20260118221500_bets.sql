CREATE TABLE public.bets
(
    id           SERIAL                   PRIMARY KEY,
    guild_id     bigint                   NOT NULL,
    bet_short_id integer                  NOT NULL,
    channel_id   bigint                   NOT NULL,
    message_id   bigint                   NOT NULL,
    author_id    bigint                   NOT NULL,
    description  text                     NOT NULL,
    expiry       timestamp with time zone NOT NULL,
    created_at   timestamp with time zone NOT NULL DEFAULT now(),
    UNIQUE (guild_id, bet_short_id)
);

CREATE INDEX idx_bets_channel_id_created_at
    ON public.bets (channel_id, created_at DESC);

CREATE TABLE public.bet_participants
(
    bet_id  integer NOT NULL REFERENCES public.bets (id) ON DELETE CASCADE,
    user_id bigint NOT NULL,
    status  text   NOT NULL DEFAULT 'accepted', -- allowed values: 'accepted', 'denied', 'watching'
    comment text   NOT NULL,
    CONSTRAINT bet_participants_status_check CHECK (status IN ('accepted', 'denied', 'watching')),
    PRIMARY KEY (bet_id, user_id)
);
