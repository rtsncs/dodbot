use super::queue::{Queue, TrackEnd};
use serenity::{
    client::Context,
    model::{
        channel::Message,
        id::{ChannelId, GuildId},
    },
    prelude::Mutex,
};
use songbird::{Call, Event, TrackEvent};
use std::{sync::Arc, time::Duration};
use tracing::error;

pub async fn voice_check(
    ctx: &Context,
    msg: &Message,
) -> Result<(Arc<Mutex<Call>>, Arc<Queue>), String> {
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

                    let queue = Queue::get(ctx, &guild_id).await;

                    Ok((handler, queue))
                } else {
                    Err("You must be in the same voice channel to use this command".to_string())
                }
            }
            None => return join(ctx, guild_id, user_channel_id, msg.channel_id).await,
        }
    } else {
        Err("You must in a voice channel to use this command.".to_string())
    }
}

pub async fn join(
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
    text_channel_id: ChannelId,
) -> Result<(Arc<Mutex<Call>>, Arc<Queue>), String> {
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

    let queue = Queue::get(ctx, &guild_id).await;
    handler.lock().await.add_global_event(
        Event::Track(TrackEvent::End),
        TrackEnd {
            queue: queue.clone(),
            channel_id: text_channel_id,
            http: ctx.http.clone(),
        },
    );

    Ok((handler, queue))
}

pub async fn react_ok(ctx: &Context, msg: &Message) {
    if let Err(why) = msg.react(&ctx.http, '✅').await {
        error!("Error reacting to message: {:?}", why);
    }
}

pub fn duration_to_string(dur: &Duration) -> String {
    let dur = dur.as_secs();
    let seconds = dur % 60;
    let minutes = (dur / 60) % 60;
    let hours = dur / 60 / 60;

    let mut string = String::new();
    if hours > 0 {
        string += &format!("{:>02}:", hours);
    }
    string += &format!("{:>02}:{:>02}", minutes, seconds);
    string
}
