mod music;

use music::commands::*;
use serenity::async_trait;
use serenity::framework::standard::{macros::group, StandardFramework};
use serenity::prelude::*;
use songbird::SerenityInit;
use std::fs::read_to_string;
use toml::Value;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: serenity::model::prelude::Ready) {
        println!("{} connected", ready.user.name);
    }
}

#[group]
#[commands(play, join, leave)]
struct Music;

#[tokio::main]
async fn main() {
    let config = read_to_string("./config.toml")
        .expect("Config file missing")
        .parse::<Value>()
        .unwrap();
    let token = config["token"].as_str().unwrap();

    let framework = StandardFramework::new()
        .configure(|c| c.with_whitespace(true).prefix("!"))
        .group(&MUSIC_GROUP);

    let mut client = Client::builder(token)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Error creating client");

    let shard_manager = client.shard_manager.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Error registering ctrl+c handler");
        shard_manager.lock().await.shutdown_all().await;
        println!("Shutting down!");
    });

    if let Err(why) = client.start().await {
        println!("Error starting client: {}", why);
    }
}
