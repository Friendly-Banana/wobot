CREATE TABLE public.reminder
(
    channel_id bigint                   NOT NULL,
    msg_id     bigint primary key,
    user_id    bigint                   NOT NULL,
    time       timestamp with time zone NOT NULL,
    content    text                     NOT NULL
);