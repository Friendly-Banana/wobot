-- update emoji usage
ALTER TABLE emoji_usage
    ADD COLUMN emoji TEXT;

-- Unicode emojis can be used directly
UPDATE emoji_usage
SET emoji = ute.unicode
FROM unicode_to_emoji ute
WHERE emoji_usage.emoji_id = ute.id;
-- custom emojis need to be converted to the format <:name:id>
UPDATE emoji_usage
SET emoji = '<:name:' || emoji_id || '>'
WHERE emoji IS NULL;

ALTER TABLE emoji_usage
    DROP CONSTRAINT emoji_usage_pkey,
    DROP COLUMN emoji_id,
    ALTER COLUMN emoji SET NOT NULL,
    ADD PRIMARY KEY (guild_id, emoji);


-- update reaction roles
ALTER TABLE reaction_roles
    ADD COLUMN emoji TEXT;

UPDATE reaction_roles
SET emoji = ute.unicode
FROM unicode_to_emoji ute
WHERE reaction_roles.emoji_id = ute.id;

UPDATE reaction_roles
SET emoji = '<:name:' || emoji_id || '>'
WHERE emoji IS NULL;

ALTER TABLE reaction_roles
    DROP CONSTRAINT reaction_roles_pkey,
    DROP COLUMN emoji_id,
    ALTER COLUMN emoji SET NOT NULL,
    ADD PRIMARY KEY (message_id, emoji);

DROP TABLE unicode_to_emoji;