-- Add migration script here
CREATE TABLE guilds
(
    guild_id            bigint PRIMARY KEY,
    prefix              text,
    round_robin         boolean NOT NULL DEFAULT false,
    mc_addresses        text[] NOT NULL DEFAULT array[]::text[],
    mc_channels         bigint[] NOT NULL DEFAULT array[]::bigint[],
    mc_names            text[] NOT NULL DEFAULT array[]::text[],
    text_channel_id     bigint,
    voice_channel_id    bigint,
    dj_role_ids         bigint[]
)
