CREATE TABLE public.features
(
    id        serial,
    name      text                     NOT NULL,
    state     bigint                   NOT NULL,
    timestamp timestamp with time zone NOT NULL DEFAULT now()
);