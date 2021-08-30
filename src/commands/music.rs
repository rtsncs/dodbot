use crate::{
    music::{
        queue::{LoopModes, Queue, QueuedTrack},
        utils,
    },
    shared_data::Spotify,
    Lavalink,
};
use regex::Regex;
use rspotify::{clients::BaseClient, model::Id as SpotifyId};
use serenity::{
    builder::CreateEmbed,
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
        return Err("You must be in voice channel".into());
    };

    match utils::join(ctx, guild_id, channel_id, msg.channel_id).await {
        Ok(_) => {
            utils::react_ok(ctx, msg).await;
        }
        Err(_) => {
            return Err("Error joining the voice channel".into());
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
        let mut queue_lock = queue.lock().await;
        queue_lock.clean_up();

        let data = ctx.data.read().await;
        let lava = data.get::<crate::Lavalink>().unwrap().clone();
        if lava.destroy(guild_id).await.is_err() || manager.remove(guild_id).await.is_err() {
            return Err("Error disconnecting".into());
        }
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
        return Err("Not in voice chat".into());
    }

    Ok(())
}

#[command]
#[aliases(p)]
#[min_args(1)]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut query = args.message().to_string();

    let is_url = query.starts_with("http");

    match utils::voice_check(ctx, msg, true).await {
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
                    return Err("Invalid spotify url".into());
                }
                if &capture[2] != "track" {
                    return Err("Use the `playlist` command to queue an album or a playlist".into());
                }
                let id = &capture[3];
                let id = SpotifyId::from_id(id)?;
                let track = spotify.track(id).await?;
                query = format!("{} - {}", track.artists[0].name, track.name);
            }
            let query_result = lava.auto_search_tracks(query).await?;

            if query_result.tracks.is_empty() {
                return Err("No matching videos found".into());
            }
            let track = query_result.tracks[0].clone();
            let info = track.info.clone();
            if queue
                .lock()
                .await
                .enqueue(QueuedTrack::new_initialized(track, msg.author.id), lava)
                .await
                .is_err()
            {
                return Err("Error queuing the track".into());
            }

            let title = info.map(|info| info.title);
            if is_url || title.is_none() {
                utils::react_ok(ctx, msg).await;
            } else {
                msg.channel_id
                    .send_message(ctx, |m| {
                        m.embed(|e| {
                            e.description(format!("{} added to the queue", title.clone().unwrap()))
                        })
                    })
                    .await?;
            }
        }
        Err(why) => {
            return Err(why.into());
        }
    }

    Ok(())
}

#[command]
#[aliases(pl)]
#[min_args(1)]
async fn playlist(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.message().to_string();
    let mut tracks: Vec<QueuedTrack> = Vec::new();

    match utils::voice_check(ctx, msg, true).await {
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
                    return Err("Invalid spotify url".into());
                }
                if &capture[2] == "track" {
                    return Err("Use the `play` command to queue a single track".into());
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
                            tracks.push(QueuedTrack::new(query, artist, length, msg.author.id));
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
                                tracks.push(QueuedTrack::new(query, artist, length, msg.author.id));
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
                    tracks.push(QueuedTrack::new_initialized(track, msg.author.id));
                }
            }

            if tracks.is_empty() {
                return Err("No matching videos found".into());
            }
            let amount = tracks.len();
            if queue
                .lock()
                .await
                .enqueue_multiple(tracks, lava)
                .await
                .is_err()
            {
                return Err("Error queuing the tracks".into());
            }

            msg.channel_id
                .send_message(ctx, |m| {
                    m.embed(|e| e.description(format!("Added {} tracks to the queue", amount)))
                })
                .await?;
        }
        Err(why) => {
            return Err(why.into());
        }
    }

    Ok(())
}

#[command]
#[min_args(1)]
async fn search(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.message();

    match utils::voice_check(ctx, msg, true).await {
        Ok((lava, queue)) => {
            let mut query_result = lava.search_tracks(query).await?;

            if query_result.tracks.is_empty() {
                return Err("No videos found".into());
            }
            query_result.tracks.truncate(5);

            let mut results = String::new();
            for (i, track) in query_result.tracks.iter().enumerate() {
                let info = track.info.as_ref().unwrap();
                let title = info.title.clone();
                let length = info.length / 1000;

                results += &format!(
                    "{}. {} [{}]\n",
                    i + 1,
                    title,
                    utils::length_to_string(length)
                );
            }

            msg.channel_id
                .send_message(ctx, |m| {
                    m.embed(|e| e.title("Search results").description(results))
                })
                .await?;
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
                                .lock()
                                .await
                                .enqueue(QueuedTrack::new_initialized(track, msg.author.id), lava)
                                .await
                                .is_err()
                            {
                                return Err("Error queuing the track".into());
                            }
                            utils::react_ok(ctx, &choice_msg).await;
                        } else {
                            return Err("Incorrect choice".into());
                        }
                    }
                    Err(_) => {
                        return Err("Incorrect choice".into());
                    }
                }
            } else {
                return Err("No song selected within 10 seconds".into());
            }
        }
        Err(why) => {
            return Err(why.into());
        }
    }

    Ok(())
}

#[command]
#[aliases(nowplaying, np, song)]
async fn songinfo(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let queue_lock = queue.lock().await;
    let data = ctx.data.read().await;
    let lava = data.get::<Lavalink>().unwrap();
    let nodes = lava.nodes().await;
    let node = nodes.get(guild_id.as_u64());

    let mut embed = CreateEmbed::default();
    embed
        .author(|a| a.name("Now playing"))
        .title("No track currently playing.");

    if let Some(node) = node {
        if let Some(track) = &node.now_playing {
            let info = track.track.info.as_ref().unwrap();
            let title = info.title.clone();

            let pos = utils::length_to_string(info.position / 1000);
            let duration = utils::length_to_string(info.length / 1000);

            let requester_id = queue_lock.current_track.clone().unwrap().requester;
            let requester = ctx.cache.member(guild_id, requester_id).await;

            embed
                .title(format!("{} ({}/{})", title, pos, duration))
                .thumbnail(format!(
                    "https://i.ytimg.com/vi/{}/hqdefault.jpg",
                    info.identifier
                ))
                .url(info.uri.clone())
                .footer(|f| {
                    if let Some(requester) = requester {
                        if let Some(avatar) = requester.user.avatar_url() {
                            f.icon_url(avatar);
                        }
                        f.text(format!("Requested by {}", requester.user.tag()))
                    } else {
                        f.text(format!("Requested by {}", requester_id.0))
                    }
                });
        }
    }

    msg.channel_id
        .send_message(ctx, |m| m.set_embed(embed))
        .await?;
    Ok(())
}

#[command]
#[aliases(q, list, ls)]
async fn queue(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut page = args.parse::<usize>().unwrap_or(1);
    if page == 0 {
        page = 1;
    }
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let queue_lock = queue.lock().await;

    let (tracklist, info) = queue_lock.tracklist(page - 1);
    let mut embed = CreateEmbed::default();
    embed.title("Queue").description(tracklist);
    if let Some((page, page_count, length)) = info {
        embed.footer(|f| {
            f.text(format!(
                "Page {}/{} | Total queue length: {}",
                page + 1,
                page_count,
                utils::length_to_string(length.as_secs())
            ))
        });
    }

    msg.channel_id
        .send_message(ctx, |m| m.set_embed(embed))
        .await?;

    Ok(())
}

#[command]
#[aliases(mq)]
async fn myqueue(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut page = args.parse::<usize>().unwrap_or(1);
    if page == 0 {
        page = 1;
    }
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let queue_lock = queue.lock().await;
    let (tracklist, info) = queue_lock.user_tracklist(msg.author.id, page - 1);
    let mut embed = CreateEmbed::default();
    embed.title("Queue").description(tracklist);
    if let Some((page, page_count, length)) = info {
        embed.footer(|f| {
            f.text(format!(
                "Page {}/{} | Total queue length: {}",
                page + 1,
                page_count,
                utils::length_to_string(length.as_secs())
            ))
        });
    }

    msg.channel_id
        .send_message(ctx, |m| m.set_embed(embed))
        .await?;

    Ok(())
}

#[command]
#[aliases(c)]
async fn clear(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let mut queue_lock = queue.lock().await;
    queue_lock.clear(msg.author.id);
    utils::react_ok(ctx, msg).await;

    Ok(())
}

#[command]
async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
    match utils::voice_check(ctx, msg, false).await {
        Ok((lava, queue)) => {
            if queue.lock().await.stop(lava).await.is_err() {
                return Err("Error stoping".into());
            }
            utils::react_ok(ctx, msg).await;
        }
        Err(why) => {
            return Err(why.into());
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
    let mut queue_lock = queue.lock().await;
    match queue_lock.remove(index - 1, msg.author.id) {
        Some(track) => {
            msg.channel_id
                .send_message(ctx, |m| {
                    m.embed(|e| {
                        e.description(format!("{} has been removed from the queue", &track.title))
                    })
                })
                .await?;
        }
        None => return Err("Index out of range".into()),
    }

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
    let mut queue_lock = queue.lock().await;
    match queue_lock.move_track(from - 1, to - 1, msg.author.id) {
        Some(track) => {
            msg.channel_id
                .send_message(ctx, |m| {
                    m.embed(|e| {
                        e.description(format!(
                            "{} has been moved to position {}",
                            &track.title, to
                        ))
                    })
                })
                .await?;
        }
        None => return Err("Index out of range".into()),
    }

    Ok(())
}

#[command]
#[min_args(2)]
async fn swap(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let first = args.parse::<usize>()?;
    let second = args.advance().parse::<usize>()?;
    let guild_id = msg.guild_id.unwrap();

    let queue = Queue::get(ctx, guild_id).await;
    let mut queue_lock = queue.lock().await;
    match queue_lock.swap(first - 1, second - 1, msg.author.id) {
        Some((first, second)) => {
            msg.channel_id
                .send_message(ctx, |m| {
                    m.embed(|e| {
                        e.description(format!(
                            "{} and {} have been swapped",
                            &first.title, &second.title,
                        ))
                    })
                })
                .await?;
        }
        None => return Err("Index out of range".into()),
    };

    Ok(())
}

#[command]
async fn skip(ctx: &Context, msg: &Message) -> CommandResult {
    match utils::voice_check(ctx, msg, false).await {
        Ok((lava, queue)) => {
            if queue.lock().await.skip(lava).await.is_err() {
                return Err("Error skipping the track".into());
            }
            utils::react_ok(ctx, msg).await;
        }
        Err(why) => {
            return Err(why.into());
        }
    }

    Ok(())
}

#[command]
#[aliases(sh)]
async fn shuffle(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    match utils::voice_check(ctx, msg, false).await {
        Ok(_) => {
            let queue = Queue::get(ctx, guild_id).await;
            let mut queue_lock = queue.lock().await;
            queue_lock.shuffle(msg.author.id);
            utils::react_ok(ctx, msg).await;
        }
        Err(why) => return Err(why.into()),
    }

    Ok(())
}

#[command]
#[min_args(1)]
async fn seek(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let position = Duration::from_secs(args.parse::<u64>().unwrap());
    let guild_id = msg.guild_id.unwrap();
    match utils::voice_check(ctx, msg, false).await {
        Ok((lava, _)) => {
            if lava.seek(guild_id, position).await.is_err() {
                return Err("Error seeking the track".into());
            }
            utils::react_ok(ctx, msg).await;
        }
        Err(why) => {
            return Err(why.into());
        }
    }

    Ok(())
}

#[command]
async fn pause(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    match utils::voice_check(ctx, msg, false).await {
        Ok((lava, _)) => {
            if lava.pause(guild_id).await.is_err() {
                return Err("Error pausing the track".into());
            }
            utils::react_ok(ctx, msg).await;
        }
        Err(why) => {
            return Err(why.into());
        }
    }

    Ok(())
}

#[command]
#[aliases(r, unpause)]
async fn resume(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    match utils::voice_check(ctx, msg, false).await {
        Ok((lava, _)) => {
            if lava.resume(guild_id).await.is_err() {
                return Err("Error resuming the track".into());
            }
            utils::react_ok(ctx, msg).await;
        }
        Err(why) => {
            return Err(why.into());
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
        _ => return Err("Invalid argument".into()),
    };
    match utils::voice_check(ctx, msg, false).await {
        Ok(_) => {
            let guild_id = msg.guild_id.unwrap();
            let queue = Queue::get(ctx, guild_id).await;
            let mut queue_lock = queue.lock().await;
            queue_lock.set_loop_mode(mode, msg.author.id);
            utils::react_ok(ctx, msg).await;
        }
        Err(why) => return Err(why.into()),
    }
    Ok(())
}

#[command]
#[aliases(vol)]
#[min_args(1)]
async fn volume(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let volume = if let Ok(volume) = args.parse::<u16>() {
        if volume > 1000 {
            return Err("Volume must be between 0% and 1000%.".into());
        }
        volume
    } else {
        return Err("Volume must be between 0% and 1000%.".into());
    };
    let guild_id = msg.guild_id.unwrap();

    match utils::voice_check(ctx, msg, false).await {
        Ok((lava, _)) => {
            lava.volume(guild_id, volume).await?;
            utils::react_ok(ctx, msg).await;
        }
        Err(why) => return Err(why.into()),
    }

    Ok(())
}
