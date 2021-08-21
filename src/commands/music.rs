use crate::{
    music::{
        queue::{LoopModes, Queue, QueuedTrack},
        utils::{self, react_ok, voice_check},
    },
    shared_data::Spotify,
    Lavalink,
};
use regex::Regex;
use rspotify::{clients::BaseClient, model::Id as SpotifyId};
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
        queue.set_loop_mode(LoopModes::None).await;

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
    let mut query = args.message().to_string();

    let is_url = query.starts_with("http");

    match utils::voice_check(ctx, msg).await {
        Ok((lava, queue)) => {
            if query.contains("open.spotify.com") {
                let data = ctx.data.read().await;
                let spotify = data.get::<Spotify>().unwrap();
                let reg = Regex::new(
                    r"^(https://open.spotify.com/)(playlist|album|track)/([a-zA-Z0-9]+)(.*)$",
                )
                .unwrap();
                let capture = reg.captures(&query).unwrap();
                if capture.len() < 3 {
                    msg.reply(ctx, "Invalid spotify url").await?;
                    return Ok(());
                }
                if &capture[2] != "track" {
                    msg.reply(
                        ctx,
                        "Use the `playlist` command to queue an album or a playlist",
                    )
                    .await?;
                    return Ok(());
                }
                let id = &capture[3];
                let id = SpotifyId::from_id(id)?;
                let track = spotify.track(id).await?;
                query = format!("{} - {}", track.artists[0].name, track.name);
            }
            let query_result = lava.auto_search_tracks(query).await?;

            if query_result.tracks.is_empty() {
                msg.reply(ctx, "No videos found").await?;
                return Ok(());
            }
            let track = query_result.tracks[0].clone();
            let info = track.info.clone();
            if queue
                .enqueue(QueuedTrack::new_initialized(track), lava)
                .await
                .is_err()
            {
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
#[min_args(1)]
async fn playlist(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.message().to_string();
    let mut tracks: Vec<QueuedTrack> = Vec::new();

    match voice_check(ctx, msg).await {
        Ok((lava, queue)) => {
            if query.contains("open.spotify.com") {
                let data = ctx.data.read().await;
                let spotify = data.get::<Spotify>().unwrap();
                let reg = Regex::new(
                    r"^(https://open.spotify.com/)(playlist|album|track)/([a-zA-Z0-9]+)(.*)$",
                )
                .unwrap();
                let capture = reg.captures(&query).unwrap();
                if capture.len() < 3 {
                    msg.reply(ctx, "Invalid spotify url").await?;
                    return Ok(());
                }
                if &capture[2] == "track" {
                    msg.reply(ctx, "Use the `play` command to queue a single track")
                        .await?;
                    return Ok(());
                }
                let id = &capture[3];
                let mut offset = 0;
                if &capture[2] == "album" {
                    let limit = 50;
                    let id = SpotifyId::from_id(id)?;
                    loop {
                        let album = spotify
                            .album_track_manual(id, Some(limit), Some(offset))
                            .await?;

                        for track in album.items {
                            let title = track.name;
                            let artist = track.artists[0].name.clone();
                            let length = track.duration;
                            let query = format!("{} - {}", &artist, &title);
                            tracks.push(QueuedTrack::new(query, artist, length));
                        }

                        if album.next.is_none() {
                            break;
                        }
                        offset += limit;
                    }
                } else {
                    let limit = 100;
                    let id = SpotifyId::from_id(id)?;
                    loop {
                        let playlist = spotify
                            .playlist_tracks_manual(id, None, None, Some(limit), Some(offset))
                            .await?;

                        for item in playlist.items {
                            if let Some(rspotify::model::PlayableItem::Track(track)) = item.track {
                                let title = track.name;
                                let artist = track.artists[0].name.clone();
                                let length = track.duration;
                                let query = format!("{} - {}", &artist, &title);
                                tracks.push(QueuedTrack::new(query, artist, length));
                            }
                        }

                        if playlist.next.is_none() {
                            break;
                        }
                        offset += limit;
                    }
                }
            } else {
                let query_result = lava.get_tracks(query).await?;
                for track in query_result.tracks {
                    tracks.push(QueuedTrack::new_initialized(track));
                }
            }

            if tracks.is_empty() {
                msg.reply(ctx, "No matches found").await?;
                return Ok(());
            }
            let amount = tracks.len();
            if queue.enqueue_multiple(tracks, lava).await.is_err() {
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
#[min_args(1)]
async fn search(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.message();

    match utils::voice_check(ctx, msg).await {
        Ok((lava, queue)) => {
            let mut query_result = lava.search_tracks(query).await?;

            if query_result.tracks.is_empty() {
                msg.reply(ctx, "No videos found").await?;
                return Ok(());
            }
            query_result.tracks.truncate(5);

            let mut results = String::new();
            for (i, track) in query_result.tracks.iter().enumerate() {
                let info = track.info.as_ref().unwrap();
                let title = info.title.clone();
                let length = info.length;

                results += &format!(
                    "{}. {} [{}]\n",
                    i + 1,
                    title,
                    utils::length_to_string(length)
                );
            }

            msg.reply(ctx, results).await?;
            if let Some(choice_msg) = msg
                .author
                .await_reply(ctx)
                .channel_id(msg.channel_id)
                .timeout(Duration::from_secs(10))
                .await
            {
                let choice = choice_msg.content.parse::<usize>();
                match choice {
                    Ok(choice) => {
                        if (1..=5).contains(&choice) {
                            let track = query_result.tracks[choice - 1].clone();
                            if queue
                                .enqueue(QueuedTrack::new_initialized(track), lava)
                                .await
                                .is_err()
                            {
                                choice_msg.reply(ctx, "Error queuing the track").await?;
                            } else {
                                react_ok(ctx, &choice_msg).await;
                            }
                        } else {
                            choice_msg.reply(ctx, "Incorrect choice").await?;
                        }
                    }
                    Err(_) => {
                        choice_msg.reply(ctx, "Incorrect choice").await?;
                    }
                }
            } else {
                msg.reply(ctx, "No song selected within 10 seconds").await?;
            }
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

            let pos = utils::length_to_string(info.position / 1000);
            let duration = utils::length_to_string(info.length / 1000);

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
            let title = &track.title;
            let duration = utils::length_to_string(track.length.as_secs());

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
#[min_args(1)]
async fn remove(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let index = args.parse::<usize>()?;
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let reply = match queue.remove(index).await {
        Some(track) => {
            format!("{} has been removed from the queue", &track.title)
        }
        None => "Index out of range".to_string(),
    };
    msg.reply(ctx, reply).await?;

    Ok(())
}

#[command]
#[aliases(move)]
#[min_args(1)]
async fn mv(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let from = args.parse::<usize>()?;
    let to = args.advance().parse::<usize>().unwrap_or(1);
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let reply = match queue.move_track(from, to).await {
        Some(track) => {
            format!("{} has been moved to position {}", &track.title, to)
        }
        None => "Index out of range".to_string(),
    };
    msg.reply(ctx, reply).await?;

    Ok(())
}

#[command]
#[min_args(2)]
async fn swap(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let first = args.parse::<usize>()?;
    let second = args.advance().parse::<usize>()?;
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let reply = match queue.swap(first, second).await {
        Some((first, second)) => {
            format!("{} and {} have been swapped", &first.title, &second.title,)
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
#[min_args(1)]
async fn seek(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let position = Duration::from_secs(args.parse::<u64>().unwrap());
    let guild_id = msg.guild_id.unwrap();
    match voice_check(ctx, msg).await {
        Ok((lava, _)) => {
            if lava.seek(guild_id, position).await.is_err() {
                msg.reply(ctx, "Error seeking the track").await?;
            } else {
                react_ok(ctx, msg).await;
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
            } else {
                react_ok(ctx, msg).await;
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
            } else {
                react_ok(ctx, msg).await;
            }
        }
        Err(why) => {
            msg.reply(ctx, why).await?;
        }
    }

    Ok(())
}

#[command]
#[aliases(loop)]
async fn repeat(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mode = match args.current() {
        Some("song") => LoopModes::Song,
        Some("queue") => LoopModes::Queue,
        Some("none") => LoopModes::None,
        None => {
            msg.reply(ctx, "Missing argument").await?;
            return Ok(());
        }
        _ => {
            msg.reply(ctx, "Invalid argument").await?;
            return Ok(());
        }
    };

    let guild_id = msg.guild_id.unwrap();
    let queue = Queue::get(ctx, guild_id).await;
    queue.set_loop_mode(mode).await;
    react_ok(ctx, msg).await;

    Ok(())
}
