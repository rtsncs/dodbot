use crate::guild::Guild;
use lavalink_rs::LavalinkClient;
use rspotify::ClientCredsSpotify;
use serenity::{model::id::GuildId, prelude::*};
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};

pub struct Guilds;
impl TypeMapKey for Guilds {
    type Value = Arc<Mutex<HashMap<GuildId, Arc<Guild>>>>;
}

pub struct Lavalink;
impl TypeMapKey for Lavalink {
    type Value = LavalinkClient;
}

pub struct Database;
impl TypeMapKey for Database {
    type Value = PgPool;
}

pub struct Spotify;
impl TypeMapKey for Spotify {
    type Value = ClientCredsSpotify;
}
