CREATE TABLE public.auto_replies
(
    user_id bigint                NOT NULL,
    keyword character varying(30) NOT NULL,
    count   bigint                NOT NULL,
    primary key (keyword, user_id)
);