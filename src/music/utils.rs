use super::queue::Queue;
use crate::Lavalink;
use lavalink_rs::LavalinkClient;
use serenity::{
    client::Context,
    model::{
        channel::Message,
        id::{ChannelId, GuildId},
    },
    prelude::Mutex,
};
use std::sync::Arc;
use tracing::{error, log::info};

pub async fn voice_check(
    ctx: &Context,
    msg: &Message,
    should_join: bool,
) -> Result<(LavalinkClient, Arc<Mutex<Queue>>), String> {
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

        if let Some(bot_channel_id) = bot_channel_id {
            if bot_channel_id == user_channel_id {
                let data = ctx.data.read().await;
                let lava = data.get::<Lavalink>().unwrap().clone();

                let queue = Queue::get(ctx, guild_id).await;
                Ok((lava, queue))
            } else {
                Err("You must be in the same voice channel to use this command".to_string())
            }
        } else {
            if should_join {
                return join(ctx, guild_id, user_channel_id, msg.channel_id).await;
            }
            Err("Not in a voice channel".to_string())
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
) -> Result<(LavalinkClient, Arc<Mutex<Queue>>), String> {
    let manager = songbird::get(ctx)
        .await
        .expect("Missing Songbird client")
        .clone();

    let (handler, info) = manager.join_gateway(guild_id, channel_id).await;

    let info = match info {
        Ok(info) => info,
        Err(why) => return Err(why.to_string()),
    };
    if let Err(why) = handler.clone().lock().await.deafen(true).await {
        return Err(why.to_string());
    }

    let queue = Queue::get(ctx, guild_id).await;
    {
        let mut queue_lock = queue.lock().await;
        queue_lock.channel_id = Some(text_channel_id);
    }

    let data = ctx.data.read().await;
    let lava_client = data.get::<Lavalink>().unwrap().clone();
    if let Err(why) = lava_client.create_session_with_songbird(&info).await {
        return Err(why.to_string());
    }

    Ok((lava_client, queue))
}

pub async fn react_ok(ctx: &Context, msg: &Message) {
    if let Err(why) = msg.react(&ctx.http, '✅').await {
        error!("Error reacting to message: {:?}", why);
    }
}

pub fn length_to_string(dur: u64) -> String {
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

pub async fn refresh_spotify_token(
    spotify: &mut rspotify::ClientCredsSpotify,
) -> Result<(), rspotify::ClientError> {
    info!("Refreshing Spotify token");
    spotify.request_token().await
}
