use crate::shared_data::ShardManagerContainer;
use async_minecraft_ping::ConnectionConfig;
use serde_json::json;
use serenity::{
    builder::CreateEmbed,
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
    let port = match address.get(1) {
        Some(port) => port.parse().unwrap_or(25565),
        None => 25565,
    };

    let config = ConnectionConfig::build(address[0]).with_port(port);
    let mut connection = config.connect().await?;
    let status = connection.status().await?;

    let motd = match status.description {
        async_minecraft_ping::ServerDescription::Plain(motd) => motd,
        async_minecraft_ping::ServerDescription::Object { text } => text,
    };

    let mut embed = CreateEmbed::default();
    embed.title(address[0]).description(format!(
        "{}\nPlayers online: {}/{}",
        motd, status.players.online, status.players.max
    ));

    if let Some(icon_base64) = &status.favicon {
        let icon_base64 = &icon_base64[22..].replace('\n', "");
        if let Ok(icon) = base64::decode(icon_base64) {
            let path = format!(
                "mc_icon_{}.png",
                args.message().replace('.', "").replace(':', "")
            );
            std::fs::write(&path, icon)?;
            embed.thumbnail(format!("attachment://{}", &path));
            msg.channel_id
                .send_files(ctx, vec![&path[..]], |m| m.set_embed(embed))
                .await?;
            let _err = std::fs::remove_file(path);
        }
    } else {
        msg.channel_id
            .send_message(ctx, |m| m.set_embed(embed))
            .await?;
    }

    Ok(())
}
