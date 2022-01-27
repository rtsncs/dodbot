use crate::error::Error;
use crate::Context;
use serenity::model::prelude::ChannelType::Voice;
use tracing::{error, info};

#[poise::command(slash_command)]
pub async fn roundrobin(
    ctx: Context<'_>,
    #[description = "On/Off"] arg: bool,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let data = ctx.data();
    let queue = data.guilds.get_queue(guild_id).await;
    let mut queue_lock = queue.lock().await;
    queue_lock.set_round_robin(arg);

    let database = &data.database;

    if let Err(why) = sqlx::query!(
        "INSERT INTO guilds (guild_id, round_robin)
        VALUES ($1, $2)
        ON CONFLICT (guild_id) DO UPDATE
            SET round_robin = $2
        ",
        guild_id.0 as i64,
        arg
    )
    .execute(database)
    .await
    {
        error!(
            "Error updating round robin setting in guild {}: {:?}",
            guild_id, why
        );
        ctx.say("Error").await?;
    } else {
        info!("Round robin setting updated in guild {}", guild_id);
        ctx.say("Done.").await?;
    }

    Ok(())
}

#[poise::command(slash_command)]
pub async fn minecraftchannel(
    ctx: Context<'_>,
    #[description = "IP address"] address: String,
    #[description = "Channel name"] name: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let guild = ctx.guild().unwrap();
    let ip: Vec<&str> = address.split(':').collect();
    let port = match ip.get(1) {
        Some(port) => port.parse().unwrap_or(25565),
        None => 25565,
    };

    let config = async_minecraft_ping::ConnectionConfig::build(ip[0]).with_port(port);
    let connection = config.connect().await?;
    let connection = connection.status().await?;

    let channel = guild
        .create_channel(ctx.discord(), |c| {
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

    let database = &ctx.data().database;
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
    .execute(database)
    .await
    {
        error!(
            "Error updating minecraft settings in guild {}: {:?}",
            guild_id.0, why
        );
        ctx.say("Error").await?;
        return Ok(());
    }

    ctx.say("Done.").await?;
    Ok(())
}
