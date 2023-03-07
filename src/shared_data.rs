use crate::{
    config::Config, error::Error, events::LavalinkHandler, guild::Guild, music::queue::Queue,
};
use genius_rs::Genius as GeniusClient;
use lavalink_rs::LavalinkClient;
use rspotify::{ClientCredsSpotify, Credentials};
use serenity::{client::bridge::gateway::ShardManager, model::id::GuildId, prelude::*};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{collections::HashMap, sync::Arc};
use tracing::info;

pub struct Guilds {
    pub inner: Arc<Mutex<HashMap<GuildId, Arc<Mutex<Guild>>>>>,
}
impl Guilds {
    pub async fn get(&self, guild_id: GuildId) -> Arc<Mutex<Guild>> {
        let inner_lock = self.inner.lock().await;
        inner_lock.get(&guild_id).unwrap().clone()
    }
    pub async fn get_queue(&self, guild_id: GuildId) -> Arc<Mutex<Queue>> {
        let guild = self.get(guild_id).await;
        let guild_lock = guild.lock().await;
        guild_lock.queue.clone()
    }
}

pub struct Data {
    pub guilds: Guilds,
    pub lavalink: LavalinkClient,
    pub database: PgPool,
    pub spotify: ClientCredsSpotify,
    pub shard_manager: Arc<Mutex<ShardManager>>,
    pub genius: GeniusClient,
}
impl Data {
    pub async fn new<U, E>(
        ctx: &Context,
        ready: &serenity::model::prelude::Ready,
        framework: &poise::Framework<U, E>,
        config: Config,
    ) -> Result<Self, Error> {
        info!("Connected to Discord as {}", ready.user.name);
        let database = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.db_string)
            .await?;
        info!("Connected to database");

        let mut guilds = HashMap::default();

        for guild in &ready.guilds {
            guilds.insert(
                guild.id,
                crate::guild::Guild::new(guild.id, &database).await,
            );
        }

        let guilds = Arc::new(Mutex::new(guilds));

        let lavalink = LavalinkClient::builder(ready.user.id.0)
            .set_host(&config.lava_address)
            .set_port(config.lava_port)
            .set_password(&config.lava_password)
            .build(LavalinkHandler {
                guilds: Guilds {
                    inner: guilds.clone(),
                },
                http: ctx.http.clone(),
            })
            .await?;
        info!("Connected to lavalink");

        let spotify_creds = Credentials {
            id: config.spotify_id,
            secret: Some(config.spotify_secret),
        };
        let spotify_config = rspotify::Config {
            token_refreshing: true,
            ..Default::default()
        };

        let mut spotify = ClientCredsSpotify::with_config(spotify_creds, spotify_config);
        spotify.request_token().await.unwrap();

        let genius = GeniusClient::new(config.genius_token);
        let shard_manager = framework.shard_manager().clone();
        let ctx = Arc::new(ctx.clone());
        let db = database.clone();

        tokio::spawn(async move {
            let db = db;
            loop {
                crate::events::update_mc_channels(ctx.clone(), &db).await;
                tokio::time::sleep(std::time::Duration::from_secs(5 * 60)).await;
            }
        });

        Ok(Self {
            database,
            guilds: Guilds { inner: guilds },
            lavalink,
            spotify,
            genius,
            shard_manager,
        })
    }
}
