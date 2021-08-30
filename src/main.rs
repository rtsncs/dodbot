#![warn(clippy::pedantic)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]

mod commands;
mod events;
mod framework;
mod framework_functions;
mod guild;
mod music;
mod shared_data;

use events::*;
use framework::*;
use framework_functions::*;
use lavalink_rs::LavalinkClient;
use rspotify::{ClientCredsSpotify, Credentials};
use serenity::{
    client::bridge::gateway::GatewayIntents, framework::standard::StandardFramework, http::Http,
    prelude::*,
};
use shared_data::*;
use songbird::SerenityInit;
use sqlx::postgres::PgPoolOptions;
use std::{boxed::Box, collections::HashMap, fs::read_to_string, sync::Arc};
use toml::Value;
use tracing::{error, info, instrument};
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

#[tokio::main]
#[instrument]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    tracing::subscriber::set_global_default(subscriber)?;

    let config = read_to_string("./config.toml")?.parse::<Value>()?;
    let token = config["token"].as_str().unwrap();
    let db_string = config["db_string"].as_str().unwrap();

    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .connect(db_string)
        .await
    {
        Ok(pool) => {
            info!("Connected to database");
            pool
        }
        Err(why) => panic!("Error connecting to database: {}", why),
    };

    let http = Http::new_with_token(token);

    let bot_id = match http.get_current_application_info().await {
        Ok(info) => info.id,
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefix("")
                .on_mention(Some(bot_id))
                .dynamic_prefix(dynamic_prefix)
                .with_whitespace(true)
                .case_insensitivity(true)
        })
        .on_dispatch_error(on_dispatch_error)
        .before(before)
        .after(after)
        .group(&MUSIC_GROUP)
        .group(&ADMIN_GROUP)
        .group(&GENERAL_GROUP);

    let mut client = Client::builder(token)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .intents({
            let mut intents = GatewayIntents::empty();
            intents.insert(GatewayIntents::GUILDS);
            intents.insert(GatewayIntents::GUILD_MESSAGES);
            intents.insert(GatewayIntents::GUILD_VOICE_STATES);

            intents
        })
        .await?;

    let guilds = Arc::new(Mutex::new(HashMap::default()));

    let lava_client = LavalinkClient::builder(bot_id)
        .set_host("127.0.0.1")
        .set_password("youshallnotpass".to_string())
        .build(LavalinkHandler {
            guilds: guilds.clone(),
            http,
        })
        .await?;

    let spotify_creds = Credentials {
        id: config["spotify_id"].as_str().unwrap().to_string(),
        secret: config["spotify_secret"].as_str().unwrap().to_string(),
    };

    let mut spotify_client = ClientCredsSpotify::new(spotify_creds);
    spotify_client.request_token().await?;

    {
        let mut data = client.data.write().await;
        data.insert::<Database>(pool);
        data.insert::<Guilds>(guilds);
        data.insert::<Lavalink>(lava_client);
        data.insert::<Spotify>(spotify_client);
        data.insert::<ShardManagerContainer>(client.shard_manager.clone());
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
        error!("Error running client: {}", why);
    }

    Ok(())
}
