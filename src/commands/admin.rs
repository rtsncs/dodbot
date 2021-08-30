use crate::{guild::Guild, music::queue::Queue, shared_data::Database};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
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
