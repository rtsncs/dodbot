use std::sync::Arc;

use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::id::{ChannelId, GuildId};
use serenity::prelude::Mutex;
use songbird::Call;

pub async fn voice_check(ctx: &Context, msg: &Message) -> Result<Arc<Mutex<Call>>, String> {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let user_channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    if let Some(user_channel_id) = user_channel_id {
        let bot_channel_id = guild
            .voice_states
            .get(&ctx.http.get_current_user().await.unwrap().id)
            .and_then(|voice_state| voice_state.channel_id);

        match bot_channel_id {
            Some(bot_channel_id) => {
                if bot_channel_id == user_channel_id {
                    let handler = songbird::get(ctx)
                        .await
                        .expect("Missing Songbird client")
                        .get(guild_id)
                        .unwrap();
                    Ok(handler)
                } else {
                    Err("You must be in the same voice channel to use this command".to_string())
                }
            }
            None => {
                return join(ctx, guild_id, user_channel_id).await;
            }
        }
    } else {
        Err("You must in a voice channel to use this command.".to_string())
    }
}

pub async fn join(
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<Arc<Mutex<Call>>, String> {
    let manager = songbird::get(ctx)
        .await
        .expect("Missing Songbird client")
        .clone();

    let (handler, result) = manager.join(guild_id, channel_id).await;

    if let Err(why) = result {
        return Err(why.to_string());
    }
    if let Err(why) = handler.clone().lock().await.deafen(true).await {
        return Err(why.to_string());
    }
    Ok(handler)
}

pub async fn react_ok(ctx: &Context, msg: &Message) {
    if let Err(why) = msg.react(&ctx.http, '✅').await {
        println!("Error reacting to message: {:?}", why);
    }
}
