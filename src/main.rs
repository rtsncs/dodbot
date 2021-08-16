#![warn(clippy::pedantic)]
#![allow(clippy::wildcard_imports)]

mod guild;
mod music;

use guild::Guild;
use lavalink_rs::{gateway::*, model::*, LavalinkClient};
use music::commands::*;
use serenity::{
    async_trait,
    framework::standard::{
        macros::{group, hook},
        StandardFramework,
    },
    http::Http,
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
    resume, playlist, repeat
)]
struct Music;

struct Guilds;
impl TypeMapKey for Guilds {
    type Value = Arc<Mutex<HashMap<GuildId, Arc<Guild>>>>;
}

struct Lavalink;
impl TypeMapKey for Lavalink {
    type Value = LavalinkClient;
}

struct LavalinkHandler {
    guilds: Arc<Mutex<HashMap<GuildId, Arc<Guild>>>>,
    http: Http,
}

#[async_trait]
impl LavalinkEventHandler for LavalinkHandler {
    async fn track_start(&self, _lava: LavalinkClient, event: TrackStart) {
        info!("Track started in guild {}", event.guild_id);
    }
    async fn track_finish(&self, lava: LavalinkClient, event: TrackFinish) {
        info!("Track finished in guild {}", event.guild_id);
        let guild_id = GuildId(event.guild_id);
        let guilds = self.guilds.lock().await;
        let guild = guilds.get(&guild_id).unwrap();
        let queue = guild.queue.clone();

        queue.play_next(lava, &self.http).await;
    }
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

    let http = Http::new_with_token(token);

    let bot_id = match http.get_current_application_info().await {
        Ok(info) => info.id,
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

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

    let guilds = Arc::new(Mutex::new(HashMap::default()));

    let lava_client = LavalinkClient::builder(bot_id)
        .set_host("127.0.0.1")
        .set_password("youshallnotpass".to_string())
        .build(LavalinkHandler {
            guilds: guilds.clone(),
            http,
        })
        .await
        .expect("Error creating lava client");

    {
        let mut data = client.data.write().await;
        data.insert::<Guilds>(guilds);
        data.insert::<Lavalink>(lava_client);
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
