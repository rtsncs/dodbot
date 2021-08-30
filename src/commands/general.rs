use crate::shared_data::ShardManagerContainer;
use async_minecraft_ping::ConnectionConfig;
use serde_json::json;
use serenity::{
    client::bridge::gateway::ShardId,
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
    prelude::*,
};
use std::time::Instant;

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    let now = Instant::now();
    reqwest::get("https://discordapp.com/api/v6/gateway").await?;
    let get_latency = now.elapsed().as_millis();

    let shard_latency = {
        let shard_manager = {
            let data = ctx.data.read().await;
            data.get::<ShardManagerContainer>().unwrap().clone()
        };
        let manager = shard_manager.lock().await;
        let runners = manager.runners.lock().await;

        if let Some(runner) = runners.get(&ShardId(ctx.shard_id)) {
            match runner.latency {
                Some(latency) => format!("{}ms", latency.as_millis()),
                None => "?ms".to_string(),
            }
        } else {
            "?ms".to_string()
        }
    };

    let map = json!({"content" : "Calculating latency..."});

    let now = Instant::now();
    let mut message = ctx.http.send_message(msg.channel_id.0, &map).await?;
    let post_latency = now.elapsed().as_millis();

    message
        .edit(ctx, |m| {
            m.content("");
            m.embed(|e| {
                e.title("Pong :ping_pong:");
                e.description(format!(
                    "Gateway: {}\nREST GET: {}ms\nREST POST: {}ms",
                    shard_latency, get_latency, post_latency
                ))
            })
        })
        .await?;

    Ok(())
}

#[command]
#[min_args(1)]
async fn minecraft(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let address: Vec<&str> = args.message().split(':').collect();

    let config = ConnectionConfig::build(address[0])
        .with_port(address.get(1).unwrap_or(&"25565").parse().unwrap_or(25565));

    let mut connection = config.connect().await?;
    let status = connection.status().await?;

    //TODO: favicon

    msg.channel_id
        .send_message(ctx, |m| {
            m.embed(|e| {
                e.title(address[0]).description(format!(
                    "Players online: {}/{}",
                    status.players.online, status.players.max
                ))
            })
        })
        .await?;

    Ok(())
}
