use serenity::{
    framework::standard::{macros::hook, CommandResult, DispatchError},
    model::prelude::*,
    prelude::*,
};
use tracing::{info, log::error};

use crate::Database;

#[hook]
pub async fn on_dispatch_error(ctx: &Context, msg: &Message, error: DispatchError) {
    match error {
        DispatchError::LackingPermissions(perm) => {
            let _err = msg
                .reply(
                    ctx,
                    format!("This command requires {} permission(s).", perm),
                )
                .await;
        }
        DispatchError::NotEnoughArguments { min, given: _ } => {
            if min == 1 {
                let _err = msg.reply(ctx, "This command requires an argument.").await;
            } else {
                let _err = msg
                    .reply(
                        ctx,
                        format!("This command requires atleast {} arguments.", min),
                    )
                    .await;
            }
        }
        _ => {
            error!("Unhandled dispatch error: {:?}", error);
        }
    }
}

#[hook]
#[tracing::instrument]
pub async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
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

#[hook]
pub async fn after(ctx: &Context, msg: &Message, cmd_name: &str, res: CommandResult) {
    if let Err(why) = res {
        error!("Error while running {} command", cmd_name);
        error!("{:?}", why);

        if msg.reply(ctx, why).await.is_err() {
            error!("Error sending message in channel {}", msg.channel_id);
        }
    }
}

#[hook]
pub async fn dynamic_prefix(ctx: &Context, msg: &Message) -> Option<String> {
    let guild_id = msg.guild_id.unwrap().0 as i64;
    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap();

    let prefix = match sqlx::query!("SELECT prefix FROM guilds WHERE guild_id = $1", guild_id)
        .fetch_one(db)
        .await
    {
        Ok(result) => result.prefix.unwrap_or_else(|| "!".to_string()),
        Err(why) => {
            error!("Error fetching prefix from database: {:?}", why);
            "!".to_string()
        }
    };

    Some(prefix)
}
