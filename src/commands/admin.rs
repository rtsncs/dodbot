use crate::{guild::Guild, music::queue::Queue, shared_data::Database};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::{channel::Message, prelude::ChannelType::Voice},
    prelude::*,
};
use tracing::{error, info};

#[command]
async fn setprefix(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let prefix = args.current().unwrap();
    let guild_id = msg.guild_id.unwrap();
    let guild_name = msg.guild(ctx).await.unwrap().name;

    let guild = Guild::get(ctx, guild_id).await;
    let mut guild_lock = guild.lock().await;
    guild_lock.prefix = prefix.to_string();

    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap();

    if let Err(why) = sqlx::query!(
        "INSERT INTO guilds (guild_id, prefix)
        VALUES ($1, $2)
        ON CONFLICT (guild_id) DO UPDATE
            SET prefix = $2",
        guild_id.0 as i64,
        prefix
    )
    .execute(db)
    .await
    {
        error!(
            "Error updating guild prefix in guild {}: {:?}",
            guild_name, why
        );
        msg.react(ctx, '❌').await?;
    } else {
        info!("Prefix updated in guild {}", guild_name);
        msg.react(ctx, '✅').await?;
    }

    Ok(())
}

#[command]
async fn roundrobin(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let round_robin = match args.message().to_lowercase().trim() {
        "true" | "on" | "enabled" => true,
        "false" | "off" | "disabled" => false,
        _ => {
            msg.reply(ctx, "Invalid argument").await?;
            return Ok(());
        }
    };
    let guild_id = msg.guild_id.unwrap();
    let guild_name = msg.guild(ctx).await.unwrap().name;

    let queue = Queue::get(ctx, guild_id).await;
    let mut queue_lock = queue.lock().await;
    queue_lock.set_round_robin(round_robin);

    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap();

    if let Err(why) = sqlx::query!(
        "INSERT INTO guilds (guild_id, round_robin)
        VALUES ($1, $2)
        ON CONFLICT (guild_id) DO UPDATE
            SET round_robin = $2
        ",
        guild_id.0 as i64,
        round_robin
    )
    .execute(db)
    .await
    {
        error!(
            "Error updating round robin setting in guild {}: {:?}",
            guild_name, why
        );
        msg.react(ctx, '❌').await?;
    } else {
        info!("Round robin setting updated in guild {}", guild_name);
        msg.react(ctx, '✅').await?;
    }

    Ok(())
}

#[command]
#[min_args(2)]
async fn minecraftchannel(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    let guild = ctx.cache.guild(guild_id).await.unwrap();
    let address = args.parse::<String>()?;
    let ip: Vec<&str> = address.split(':').collect();
    let port = match ip.get(1) {
        Some(port) => port.parse().unwrap_or(25565),
        None => 25565,
    };
    let name = args.advance().remains().unwrap().to_string();

    let config = async_minecraft_ping::ConnectionConfig::build(ip[0]).with_port(port);
    let connection = config.connect().await?;
    let connection = connection.status().await?;

    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap();

    let channel = guild
        .create_channel(ctx, |c| {
            c.kind(Voice).name(name.replace(
                '$',
                &format!(
                    "{}/{}",
                    connection.status.players.online, connection.status.players.max
                ),
            ))
        })
        .await?;
    let channel_id = channel.id.0 as i64;

    if let Err(why) = sqlx::query!(
        "INSERT INTO guilds (guild_id, mc_addresses, mc_channels, mc_names)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (guild_id) DO UPDATE
            SET mc_addresses = guilds.mc_addresses || $2,
                mc_channels = guilds.mc_channels || $3,
                mc_names = guilds.mc_names || $4",
        guild_id.0 as i64,
        &vec![address],
        &vec![channel_id],
        &vec![name],
    )
    .execute(db)
    .await
    {
        error!(
            "Error updating minecraft settings in guild {}: {:?}",
            msg.guild_id.unwrap().0,
            why
        );
        msg.react(ctx, '❌').await?;
        return Ok(());
    }

    msg.react(ctx, '✅').await?;
    Ok(())
}
