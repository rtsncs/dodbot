use crate::Lavalink;

use super::{
    queue::Queue,
    utils::{self, voice_check},
};
use serenity::{
    client::Context,
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
};
use std::time::Duration;

#[command]
#[aliases(connect)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let channel_id = if let Some(channel_id) = channel_id {
        channel_id
    } else {
        msg.reply(ctx, "You must be in voice channel").await?;
        return Ok(());
    };

    match utils::join(ctx, guild_id, channel_id, msg.channel_id).await {
        Ok(_) => {
            utils::react_ok(ctx, msg).await;
        }
        Err(_) => {
            msg.reply(ctx, "Error joining the voice channel").await?;
        }
    }

    Ok(())
}

#[command]
#[aliases(disconnect, dc)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird client missing")
        .clone();

    let has_handler = manager.get(guild_id).is_some();
    let bot_id = ctx
        .http
        .get_current_user()
        .await
        .expect("Error accessing bot id")
        .id;

    if has_handler {
        let queue = Queue::get(ctx, guild_id).await;
        queue.clear().await;

        let data = ctx.data.read().await;
        let lava = data.get::<crate::Lavalink>().unwrap().clone();
        if lava.destroy(guild_id).await.is_err() || manager.remove(guild_id).await.is_err() {
            msg.reply(ctx, "Error disconnecting").await?;
        } else {
            utils::react_ok(ctx, msg).await;
        }
    } else if guild.voice_states.get(&bot_id).is_some() {
        guild
            .member(ctx, bot_id)
            .await
            .unwrap()
            .disconnect_from_voice(ctx)
            .await?;

        utils::react_ok(ctx, msg).await;
    } else {
        msg.reply(ctx, "Not in voice chat").await?;
    }

    Ok(())
}

#[command]
#[aliases(p)]
#[min_args(1)]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.message();

    let is_url = query.starts_with("http");

    match utils::voice_check(ctx, msg).await {
        Ok((lava, queue)) => {
            let query_result = lava.auto_search_tracks(query).await?;

            if query_result.tracks.is_empty() {
                msg.reply(ctx, "No videos found").await?;
                return Ok(());
            }
            let track = query_result.tracks[0].clone();
            let info = track.info.clone();
            if queue.enqueue(track, lava).await.is_err() {
                msg.reply(ctx, "Error queuing the track").await?;
                return Ok(());
            }

            let title = info.map(|info| info.title);
            if is_url || title.is_none() {
                utils::react_ok(ctx, msg).await;
            } else {
                msg.reply(
                    ctx,
                    format!("{} added to the queue", title.clone().unwrap()),
                )
                .await?;
            }
        }
        Err(why) => {
            msg.reply(ctx, why).await?;
        }
    }

    return Ok(());
}

#[command]
#[aliases(pl)]
async fn playlist(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.message();

    match voice_check(ctx, msg).await {
        Ok((lava, queue)) => {
            let query_result = lava.get_tracks(query).await?;

            if query_result.tracks.is_empty() {
                msg.reply(ctx, "No videos found").await?;
                return Ok(());
            }
            let amount = query_result.tracks.len();
            if queue
                .enqueue_multiple(query_result.tracks, lava)
                .await
                .is_err()
            {
                msg.reply(ctx, "Error queuing the tracks").await?;
                return Ok(());
            }

            msg.reply(ctx, format!("Added {} tracks to the queue", amount))
                .await?;
        }
        Err(why) => {
            msg.reply(ctx, why).await?;
        }
    }

    Ok(())
}

#[command]
#[aliases(nowplaying, np, song)]
async fn songinfo(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();

    // let queue = Queue::get(ctx, guild_id).await;
    let data = ctx.data.read().await;
    let lava = data.get::<Lavalink>().unwrap();
    let nodes = lava.nodes().await;
    let node = nodes.get(guild_id.as_u64());

    if let Some(node) = node {
        if let Some(track) = &node.now_playing {
            let info = track.track.info.as_ref().unwrap();
            let title = info.title.clone();

            let pos = utils::duration_to_string(info.position);
            let duration = utils::duration_to_string(info.length);

            msg.reply(
                ctx,
                format!("Now playing: {} ({}/{})", title, pos, duration),
            )
            .await?;
            return Ok(());
        }
    }

    msg.reply(ctx, "No track currently playing").await?;
    Ok(())
}

#[command]
#[aliases(q, list, ls)]
async fn queue(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let tracks = queue.tracklist().await;

    if tracks.len() < 2 {
        msg.reply(ctx, "The queue is empty.").await?;
    } else {
        let mut tracklist = String::new();

        for (i, track) in tracks.iter().enumerate() {
            if i == 0 {
                continue;
            }
            let title = &track.info.as_ref().unwrap().title;
            let duration = utils::duration_to_string(track.info.as_ref().unwrap().length);

            tracklist += &format!("{}. {} ({})\n", i, title, duration);
        }

        msg.reply(ctx, tracklist).await?;
    }
    Ok(())
}

#[command]
#[aliases(c)]
async fn clear(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    queue.clear().await;
    utils::react_ok(ctx, msg).await;

    Ok(())
}

#[command]
async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
    match voice_check(ctx, msg).await {
        Ok((lava, queue)) => {
            if queue.stop(lava).await.is_err() {
                msg.reply(ctx, "Error stopping").await?;
            } else {
                utils::react_ok(ctx, msg).await;
            }
        }
        Err(why) => {
            msg.reply(ctx, why).await?;
        }
    }
    Ok(())
}

#[command]
#[aliases(delete, r, d, rm)]
async fn remove(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let index = args.parse::<usize>()?;
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let reply = match queue.remove(index).await {
        Some(track) => {
            format!(
                "{} has been removed from the queue",
                track.info.unwrap().title
            )
        }
        None => "Index out of range".to_string(),
    };
    msg.reply(ctx, reply).await?;

    Ok(())
}

#[command]
#[aliases(move)]
async fn mv(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let from = args.parse::<usize>()?;
    let to = args.advance().parse::<usize>().unwrap_or(1);
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let reply = match queue.move_track(from, to).await {
        Some(track) => {
            format!(
                "{} has been moved to position {}",
                track.info.unwrap().title,
                to
            )
        }
        None => "Index out of range".to_string(),
    };
    msg.reply(ctx, reply).await?;

    Ok(())
}

#[command]
async fn swap(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let first = args.parse::<usize>()?;
    let second = args.advance().parse::<usize>()?;
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let reply = match queue.swap(first, second).await {
        Some((first, second)) => {
            format!(
                "{} and {} have been swapped",
                first.info.unwrap().title,
                second.info.unwrap().title,
            )
        }
        None => "Index out of range".to_string(),
    };
    msg.reply(ctx, reply).await?;

    Ok(())
}

#[command]
async fn skip(ctx: &Context, msg: &Message) -> CommandResult {
    match voice_check(ctx, msg).await {
        Ok((lava, queue)) => {
            if queue.skip(lava).await.is_err() {
                msg.reply(ctx, "Error skipping the track").await?;
            } else {
                utils::react_ok(ctx, msg).await;
            }
        }
        Err(why) => {
            msg.reply(ctx, why).await?;
        }
    }

    Ok(())
}

#[command]
#[aliases(sh)]
async fn shuffle(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    let queue = Queue::get(ctx, guild_id).await;
    queue.shuffle().await;
    utils::react_ok(ctx, msg).await;

    Ok(())
}

#[command]
async fn seek(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let position = Duration::from_secs(args.parse::<u64>().unwrap());
    let guild_id = msg.guild_id.unwrap();
    match voice_check(ctx, msg).await {
        Ok((lava, _)) => {
            if lava.seek(guild_id, position).await.is_err() {
                msg.reply(ctx, "Error seeking the track").await?;
            }
        }
        Err(why) => {
            msg.reply(ctx, why).await?;
        }
    }

    Ok(())
}

#[command]
async fn pause(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    match voice_check(ctx, msg).await {
        Ok((lava, _)) => {
            if lava.pause(guild_id).await.is_err() {
                msg.reply(ctx, "Error pausing the track").await?;
            }
        }
        Err(why) => {
            msg.reply(ctx, why).await?;
        }
    }

    Ok(())
}

#[command]
#[aliases(r, unpause)]
async fn resume(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    match voice_check(ctx, msg).await {
        Ok((lava, _)) => {
            if lava.resume(guild_id).await.is_err() {
                msg.reply(ctx, "Error resuming the track").await?;
            }
        }
        Err(why) => {
            msg.reply(ctx, why).await?;
        }
    }

    Ok(())
}
