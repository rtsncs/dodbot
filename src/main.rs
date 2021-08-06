mod guild;
mod music;

use guild::Guild;
use music::commands::*;
use serenity::{
    async_trait,
    framework::standard::{
        macros::{group, hook},
        StandardFramework,
    },
    model::{channel::Message, guild::GuildStatus, id::GuildId},
    prelude::*,
};
use songbird::SerenityInit;
use std::{collections::HashMap, fs::read_to_string, sync::Arc};
use toml::Value;
use tracing::{error, info, instrument};

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: serenity::model::prelude::Ready) {
        info!("{} connected", ready.user.name);

        let data = ctx.data.read().await;
        let guilds = data.get::<Guilds>().expect("Guilds missing");
        let mut guilds_lock = guilds.lock().await;

        for guild in ready.guilds {
            let guild_id = match guild {
                GuildStatus::OnlineGuild(g) => g.id,
                GuildStatus::OnlinePartialGuild(g) => g.id,
                GuildStatus::Offline(g) => g.id,
                _ => continue,
            };

            guilds_lock.insert(guild_id, Guild::new(guild_id));
        }
    }
}

#[hook]
#[instrument]
async fn before(_: &Context, msg: &Message, command_name: &str) -> bool {
    info!(
        "Got command '{}' by user '{}'",
        command_name, msg.author.name
    );
    true
}

#[group]
#[only_in(guilds)]
#[commands(
    play, join, leave, songinfo, queue, clear, stop, remove, mv, swap, skip, shuffle, seek, pause,
    resume
)]
struct Music;

struct Guilds;
impl TypeMapKey for Guilds {
    type Value = Arc<Mutex<HashMap<GuildId, Arc<Guild>>>>;
}

#[tokio::main]
#[instrument]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = read_to_string("./config.toml")
        .expect("Config file missing")
        .parse::<Value>()
        .unwrap();
    let token = config["token"].as_str().unwrap();

    let framework = StandardFramework::new()
        .configure(|c| c.with_whitespace(true).prefix("!"))
        .before(before)
        .group(&MUSIC_GROUP);

    let mut client = Client::builder(token)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<Guilds>(Arc::new(Mutex::new(HashMap::default())));
    }

    let shard_manager = client.shard_manager.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Error registering ctrl+c handler");
        info!("Shutting down!");
        shard_manager.lock().await.shutdown_all().await;
    });

    if let Err(why) = client.start().await {
        error!("Error starting client: {}", why);
    }
}
