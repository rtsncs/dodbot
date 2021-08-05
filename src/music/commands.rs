use super::{queue::Queue, utils};
use serenity::{
    client::Context,
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
};
use songbird::{create_player, input::Restartable};

#[command]
#[aliases(connect)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let channel_id = match channel_id {
        Some(channel_id) => channel_id,
        None => {
            msg.reply(ctx, "You must be in voice channel").await?;
            return Ok(());
        }
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
        let queue = Queue::get(ctx, &guild_id).await;
        queue.stop();
        manager.remove(guild_id).await?;
        utils::react_ok(ctx, msg).await;
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
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.message();

    let is_url = query.starts_with("http");

    match utils::voice_check(ctx, msg).await {
        Ok((handler, queue)) => {
            let source = match Restartable::ytdl_search(query, true).await {
                Ok(source) => source,
                Err(why) => {
                    msg.reply(ctx, format!("{:?}", why)).await?;
                    return Ok(());
                }
            };

            let track = create_player(source.into());
            let title = &track.1.metadata().title.clone();

            {
                queue.enqueue(track.0, handler.lock().await);
            }

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
#[aliases(nowplaying, np, song)]
async fn songinfo(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, &guild_id).await;

    match queue.current() {
        Some(track) => {
            let metadata = track.metadata().clone();
            let state = track.get_info().await.expect("Track state");
            let title = metadata.title.unwrap();

            let pos = utils::duration_to_string(&state.position);
            let duration = utils::duration_to_string(&metadata.duration.unwrap());

            msg.reply(
                ctx,
                format!("Now playing: {} ({}/{})", title, pos, duration),
            )
            .await?;
        }
        None => {
            msg.reply(ctx, "No track currently playing").await?;
        }
    }

    Ok(())
}

#[command]
#[aliases(q, list, ls)]
async fn queue(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, &guild_id).await;
    let tracks = queue.tracklist();

    if tracks.len() < 2 {
        msg.reply(ctx, "The queue is empty.").await?;
    } else {
        let mut tracklist = String::new();

        for (i, track) in tracks.iter().enumerate() {
            if i == 0 {
                continue;
            }
            let metadata = track.metadata().clone();
            let title = metadata.title.unwrap();
            let duration = utils::duration_to_string(&metadata.duration.unwrap());

            tracklist += &format!("{}. {} ({})\n", i, title, duration);
        }

        msg.reply(ctx, tracklist).await?;
    }
    Ok(())
}
