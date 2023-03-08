use crate::{error::Error, Context};
use async_minecraft_ping::ConnectionConfig;
use serenity::{builder::CreateEmbed, client::bridge::gateway::ShardId};
use std::time::Instant;
use systemstat::{saturating_sub_bytes, Platform, System};

#[poise::command(slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let now = Instant::now();
    reqwest::get("https://discordapp.com/api/v6/gateway").await?;
    let get_latency = now.elapsed().as_millis();

    let shard_latency = {
        let shard_manager = ctx.data().shard_manager.lock().await;
        let runners = shard_manager.runners.lock().await;

        if let Some(runner) = runners.get(&ShardId(ctx.serenity_context().shard_id)) {
            match runner.latency {
                Some(latency) => format!("{}ms", latency.as_millis()),
                None => "?ms".to_string(),
            }
        } else {
            "?ms".to_string()
        }
    };

    let now = Instant::now();
    let reply_handle = ctx.say("Calculating latency...").await?;
    let post_latency = now.elapsed().as_millis();

    reply_handle
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

#[poise::command(slash_command)]
pub async fn minecraft(
    ctx: Context<'_>,
    #[description = "IP adress"] args: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let address: Vec<&str> = args.split(':').collect();
    let port = match address.get(1) {
        Some(port) => port.parse().unwrap_or(25565),
        None => 25565,
    };

    let config = ConnectionConfig::build(address[0]).with_port(port);
    let connection = config.connect().await?;
    let connection = connection.status().await?;

    let motd = match connection.status.description {
        async_minecraft_ping::ServerDescription::Plain(motd) => motd,
        async_minecraft_ping::ServerDescription::Object { text } => text,
    };

    let mut embed = CreateEmbed::default();
    embed.title(address[0]).description(format!(
        "{}\nPlayers online: {}/{}",
        motd, connection.status.players.online, connection.status.players.max
    ));
    if let Some(icon_base64) = &connection.status.favicon {
        let icon_base64 = &icon_base64[22..].replace('\n', "");
        if let Ok(icon) = base64::decode(icon_base64) {
            let path = format!("mc_icon_{}.png", args.replace(['.', ':'], ""));
            if std::fs::write(&path, icon).is_ok() {
                embed.thumbnail(format!("attachment://{}", &path));
                ctx.send(|m| {
                    m.attachment(path.as_str().into());
                    m.embeds.push(embed);
                    m
                })
                .await?;
                let _err = std::fs::remove_file(path);
                return Ok(());
            }
        }
    }
    ctx.send(|m| {
        m.embeds.push(embed);
        m
    })
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Command to show help about"] command: Option<String>,
) -> Result<(), Error> {
    let config = poise::builtins::HelpConfiguration {
        ..Default::default()
    };
    poise::builtins::help(ctx, command.as_deref(), config).await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn vps(ctx: Context<'_>) -> Result<(), Error> {
    let sys = System::new();
    let memory = match sys.memory() {
        Ok(mem) => format!(
            "\nMemory: {} used / {} total",
            saturating_sub_bytes(mem.total, mem.free),
            mem.total,
        ),
        Err(err) => format!("\nMemory: error: {err}"),
    };
    let cpu_load = match sys.load_average() {
        Ok(load) => format!(
            "\nLoad average: {} {} {}",
            load.one, load.five, load.fifteen
        ),
        Err(err) => format!("\nLoad average: error: {err}"),
    };
    let uptime = match sys.uptime() {
        Ok(uptime) => {
            const MINUTE: u64 = 60;
            const HOUR: u64 = MINUTE * 60;
            const DAY: u64 = HOUR * 24;
            let seconds = uptime.as_secs();
            let str = if uptime.as_secs() >= DAY {
                format!(
                    "{}d{}h{}m",
                    seconds / DAY,
                    seconds % DAY / HOUR,
                    seconds % HOUR / MINUTE
                )
            } else if seconds >= HOUR {
                format!("{}h{}m", seconds / HOUR, seconds % HOUR / MINUTE,)
            } else if seconds >= MINUTE {
                format!("{}m{}s", seconds / MINUTE, seconds % MINUTE)
            } else {
                format!("{}s", seconds)
            };
            "\nUptime: ".to_string() + &str
        }
        Err(err) => format!("\nUptime: error: {err}"),
    };

    ctx.send(|m| {
        m.embed(|e| {
            e.title("System stats")
                .description(memory + &cpu_load + &uptime)
        })
    })
    .await?;

    Ok(())
}
