use crate::guild::Guild;
use crate::Context;
use tracing::{info, log::error};

// #[hook]
// pub async fn on_dispatch_error(ctx: &Context, msg: &Message, error: DispatchError) {
//     let why = match error {
//         DispatchError::LackingPermissions(perm) => {
//             format!("This command requires {} permission(s).", perm)
//         }
//         DispatchError::NotEnoughArguments { min, given: _ } => {
//             if min == 1 {
//                 "This command requires an argument.".to_string()
//             } else {
//                 format!("This command requires atleast {} arguments.", min)
//             }
//         }
//         _ => {
//             error!("Unhandled dispatch error: {:?}", error);
//             return;
//         }
//     };
//     if msg
//         .channel_id
//         .send_message(ctx, |m| {
//             m.embed(|e| e.title("Error").description(why).color(Color::RED))
//         })
//         .await
//         .is_err()
//     {
//         error!("Error sending message in channel {}", msg.channel_id);
//     }
// }

pub fn before(ctx: Context) {
    let guild_name = match ctx.guild() {
        Some(guild) => guild.name,
        None => "Direct Message".to_string(),
    };
    info!(
        "Got command '{}' by user '{}' in guild '{}'",
        ctx.command().name,
        ctx.author().name,
        guild_name
    );
}

// pub async fn after(ctx: &Context, msg: &Message, cmd_name: &str, res: CommandResult) {
//     if let Err(why) = res {
//         error!("Error while running {} command", cmd_name);
//         error!("{:?}", why);
//
//         if msg
//             .channel_id
//             .send_message(ctx, |m| {
//                 m.embed(|e| e.title("Error").description(why).color(Color::RED))
//             })
//             .await
//             .is_err()
//         {
//             error!("Error sending message in channel {}", msg.channel_id);
//         }
//     }
// }
//
// #[hook]
// pub async fn dynamic_prefix(ctx: &Context, msg: &Message) -> Option<String> {
//     let guild_id = msg.guild_id.unwrap();
//     let guild = Guild::get(ctx, guild_id).await;
//     let guild_lock = guild.lock().await;
//
//     Some(guild_lock.prefix.clone())
// }
