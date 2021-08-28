-- Add migration script here
CREATE TABLE guilds
(
    guild_id            bigint PRIMARY KEY,
    prefix              text,
    round_robin         boolean NOT NULL DEFAULT false,
    minecraft_address   text,
    minecraft_channel   bigint,
    text_channel_id     bigint,
    voice_channel_id    bigint,
    dj_role_ids         bigint[]
)
