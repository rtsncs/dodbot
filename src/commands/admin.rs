use crate::shared_data::Database;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
    prelude::*,
};
use tracing::{error, info};

#[command]
async fn setprefix(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let prefix = args.current().unwrap();
    let guild_id = msg.guild_id.unwrap().0 as i64;
    let guild_name = msg.guild(ctx).await.unwrap().name;

    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap();

    if let Err(why) = sqlx::query!(
        "INSERT INTO guilds (guild_id, prefix)
        VALUES ($1, $2)
        ON CONFLICT (guild_id) DO UPDATE
            SET prefix = $2",
        guild_id,
        prefix
    )
    .execute(db)
    .await
    {
        error!(
            "Error updating guild prefix in guild {}: {:?}",
            guild_name, why
        );
        msg.reply(ctx, "There was an error executing this command.")
            .await?;
    } else {
        info!("Prefix updated in guild {}", guild_name);
        msg.react(ctx, '✅').await?;
    }

    Ok(())
}
