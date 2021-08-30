use crate::guild::Guild;
use lavalink_rs::LavalinkClient;
use rspotify::ClientCredsSpotify;
use serenity::{client::bridge::gateway::ShardManager, model::id::GuildId, prelude::*};
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};

pub struct Guilds;
impl TypeMapKey for Guilds {
    type Value = Arc<Mutex<HashMap<GuildId, Arc<Mutex<Guild>>>>>;
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

pub struct ShardManagerContainer;
impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}
