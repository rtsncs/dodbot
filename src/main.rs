#![warn(clippy::pedantic)]
#![allow(clippy::wildcard_imports)]

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
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

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
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
    let guild_name = match msg.guild(ctx).await {
        Some(guild) => guild.name,
        None => "Direct Message".to_string(),
    };
    info!(
        "Got command '{}' by user '{}' in guild '{}'",
        command_name, msg.author.name, guild_name
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
    let file_appender = tracing_appender::rolling::daily("./logs/", "dodbot_log");
    let (file_appender, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(fmt::layer().with_writer(std::io::stdout).compact())
        .with(
            fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .compact(),
        );

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set global collector");

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
