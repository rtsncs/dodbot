use crate::{
    error::Error,
    music::{
        queue::{LoopModes, QueuedTrack},
        utils,
    },
    Context,
};
use regex::Regex;
use rspotify::{
    clients::BaseClient,
    model::{AlbumId, PlaylistId, TrackId},
};
use serenity::{
    builder::CreateEmbed, model::application::component::ButtonStyle,
    model::application::interaction::InteractionResponseType,
};
use std::time::Duration;

#[poise::command(slash_command, category = "Music")]
pub async fn join(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild().unwrap();

    let channel_id = guild
        .voice_states
        .get(&ctx.author().id)
        .and_then(|voice_state| voice_state.channel_id);

    match channel_id {
        Some(id) => {
            utils::join(&ctx, guild.id, id, ctx.channel_id()).await?;
            ctx.say(format!("Joined <#{id}>")).await?;
        }
        None => {
            return Err(Error::Join("You must be in a voice channel.".to_string()));
        }
    }

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird client missing")
        .clone();

    let has_handler = manager.get(guild.id).is_some();
    let bot_id = ctx.serenity_context().cache.current_user_id();

    if has_handler {
        let data = ctx.data();
        let queue = data.guilds.get_queue(guild.id).await;
        let mut queue_lock = queue.lock().await;
        queue_lock.clean_up();
        let lava = &data.lavalink;

        lava.destroy(guild.id).await?;
        manager.remove(guild.id).await?;
    } else if guild.voice_states.get(&bot_id).is_some() {
        guild
            .member(ctx, bot_id)
            .await
            .unwrap()
            .disconnect_from_voice(ctx)
            .await?;
    } else {
        return Err("Not in voice chat".into());
    }
    ctx.say("Disconnected").await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Name/link to a song"] mut query: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let (lava, queue) = utils::voice_check(&ctx, true).await?;
    let data = ctx.data();
    if query.contains("open.spotify.com") {
        let spotify = &data.spotify;
        let reg =
            Regex::new(r"^(https://open.spotify.com/)(playlist|album|track)/([a-zA-Z0-9]+)(.*)$")
                .unwrap();
        let capture = reg.captures(&query).unwrap();
        if capture.len() < 3 {
            return Err("Invalid spotify url".into());
        }
        if &capture[2] != "track" {
            return Err("Use the `playlist` command to queue an album or a playlist".into());
        }
        let id = &capture[3];
        let id = TrackId::from_id(id)?;
        let track = spotify.track(id).await?;
        query = format!("{} - {}", track.artists[0].name, track.name);
    }
    let mut query_result = lava.auto_search_tracks(query).await?;

    if query_result.tracks.is_empty() {
        return Err("No matching videos found".into());
    }
    let track = query_result.tracks.remove(0);
    let info = track.info.clone();
    queue
        .lock()
        .await
        .enqueue(QueuedTrack::new_initialized(track, ctx.author().id), lava)
        .await?;

    let title = info.map(|info| info.title);

    ctx.send(|m| {
        m.embed(|e| {
            e.description(format!(
                "{} added to the queue",
                title.clone().unwrap_or_else(|| "Track".to_string())
            ))
        })
    })
    .await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn playlist(
    ctx: Context<'_>,
    #[description = "Playlist URL"] query: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let mut tracks: Vec<QueuedTrack> = Vec::new();

    let (lava, queue) = utils::voice_check(&ctx, true).await?;
    let user_id = ctx.author().id;
    if query.contains("open.spotify.com") {
        let data = ctx.data();
        let spotify = &data.spotify;
        let reg =
            Regex::new(r"^(https://open.spotify.com/)(playlist|album|track)/([a-zA-Z0-9]+)(.*)$")
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
            let id = AlbumId::from_id(id)?;
            loop {
                let album = spotify
                    .album_track_manual(id.clone(), Some(limit), Some(offset))
                    .await?;

                for track in album.items {
                    let title = track.name;
                    let artist = track.artists[0].name.clone();
                    let length = track.duration;
                    let query = format!("{} - {}", &artist, &title);
                    tracks.push(QueuedTrack::new(query, artist, length, user_id));
                }

                if album.next.is_none() {
                    break;
                }
                offset += limit;
            }
        } else {
            let limit = 100;
            let id = PlaylistId::from_id(id)?;

            loop {
                let playlist = spotify
                    .playlist_items_manual(id.clone(), None, None, Some(limit), Some(offset))
                    .await?;

                for item in playlist.items {
                    if let Some(rspotify::model::PlayableItem::Track(track)) = item.track {
                        let title = track.name;
                        let artist = track.artists[0].name.clone();
                        let length = track.duration;
                        let query = format!("{} - {}", &artist, &title);
                        tracks.push(QueuedTrack::new(query, artist, length, user_id));
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
            tracks.push(QueuedTrack::new_initialized(track, user_id));
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

    ctx.send(|m| m.embed(|e| e.description(format!("Added {} tracks to the queue", amount))))
        .await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn search(
    ctx: Context<'_>,
    #[description = "Search query"] query: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let (lava, queue) = utils::voice_check(&ctx, true).await?;
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

    let uuid = ctx.id() as usize;
    let handle = ctx
        .send(|m| {
            m.embed(|e| e.title("Search results").description(results))
                .components(|c| {
                    c.create_action_row(|r| {
                        for i in 0..query_result.tracks.len() {
                            r.create_button(|b| {
                                b.style(ButtonStyle::Primary)
                                    .label(i + 1)
                                    .custom_id(uuid + i)
                            });
                        }
                        r
                    })
                })
        })
        .await?;
    let msg = handle.message().await?;
    let user_id = ctx.author().id;

    if let Some(mci) = serenity::collector::CollectComponentInteraction::new(ctx)
        .author_id(user_id)
        .message_id(msg.id)
        .collect_limit(1)
        .timeout(Duration::from_secs(30))
        .await
    {
        let choice = mci.data.custom_id.parse::<usize>().unwrap() - uuid;
        let track = query_result.tracks.remove(choice);
        let info = track.info.clone();
        let title = info.map(|info| info.title);
        queue
            .lock()
            .await
            .enqueue(QueuedTrack::new_initialized(track, user_id), lava)
            .await?;

        mci.create_interaction_response(ctx, |r| {
            r.kind(InteractionResponseType::UpdateMessage)
                .interaction_response_data(|d| {
                    d.content(format!(
                        "{} added to queue.",
                        title.unwrap_or_else(|| "Track".to_string())
                    ))
                    .components(|c| c.set_action_rows(Vec::default()))
                    .set_embeds([])
                })
        })
        .await?;
    }

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn nowplaying(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let data = ctx.data();

    let queue = data.guilds.get_queue(guild_id).await;
    let queue_lock = queue.lock().await;
    let lava = &data.lavalink;
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
            let requester = ctx.serenity_context().cache.member(guild_id, requester_id);

            let bar1 = ((info.position as f32 / info.length as f32) * 19.) as usize;
            let bar2 = 19 - bar1;
            let progress_bar = "▬".repeat(bar1) + "🔘" + &"▬".repeat(bar2);

            embed
                .title(title)
                .thumbnail(format!(
                    "https://i.ytimg.com/vi/{}/hqdefault.jpg",
                    info.identifier
                ))
                .url(info.uri.clone())
                .description(format!(
                    "{}\n{}\n{}/{}",
                    info.author, progress_bar, pos, duration
                ))
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

    ctx.send(|m| {
        m.embeds.push(embed);
        m
    })
    .await?;
    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn queue(
    ctx: Context<'_>,
    #[description = "Page"]
    #[min = 1]
    page: Option<usize>,
) -> Result<(), Error> {
    let mut page = page.unwrap_or(1);
    let mut page_count = 1;
    let guild_id = ctx.guild_id().unwrap();
    let data = ctx.data();
    let queue = data.guilds.get_queue(guild_id).await;

    let queue_lock = queue.lock().await;
    let (tracklist, info) = queue_lock.tracklist(page - 1);
    drop(queue_lock);

    let mut embed = CreateEmbed::default();
    embed.title("Queue").description(tracklist);
    if let Some((page_ret, page_count_ret, track_count, length)) = info {
        page = page_ret + 1;
        page_count = page_count_ret;
        embed.footer(|f| {
            f.text(format!(
                "Page {}/{} | Total queue length: {} {} ({})",
                page,
                page_count,
                track_count,
                if track_count == 1 { "track" } else { "tracks" },
                utils::length_to_string(length.as_secs())
            ))
        });
    }

    let uuid = ctx.id() as usize;
    let handle = ctx
        .send(|m| {
            m.embeds.push(embed);
            if page_count > 1 {
                m.components(|c| {
                    c.create_action_row(|r| {
                        r.create_button(|b| {
                            b.style(ButtonStyle::Primary).label("<").custom_id(uuid)
                        });
                        r.create_button(|b| {
                            b.style(ButtonStyle::Primary).label(">").custom_id(uuid + 1)
                        });
                        r
                    })
                });
            }
            m
        })
        .await?;
    let msg = handle.message().await?;
    let user_id = ctx.author().id;

    while let Some(mci) = serenity::collector::CollectComponentInteraction::new(ctx)
        .author_id(user_id)
        .message_id(msg.id)
        .timeout(Duration::from_secs(30))
        .await
    {
        let choice = mci.data.custom_id.parse::<usize>().unwrap() - uuid;
        if choice == 0 {
            if page >= 2 {
                page -= 1;
            }
        } else {
            page += 1
        };

        let queue_lock = queue.lock().await;
        let (tracklist, info) = queue_lock.tracklist(page - 1);
        drop(queue_lock);

        let mut embed = CreateEmbed::default();
        embed.title("Queue").description(tracklist);
        if let Some((page_ret, page_count_ret, track_count, length)) = info {
            page = page_ret + 1;
            page_count = page_count_ret;
            embed.footer(|f| {
                f.text(format!(
                    "Page {}/{} | Total queue length: {} {} ({})",
                    page,
                    page_count,
                    track_count,
                    if track_count == 1 { "track" } else { "tracks" },
                    utils::length_to_string(length.as_secs())
                ))
            });

            mci.create_interaction_response(ctx, |r| {
                r.kind(InteractionResponseType::UpdateMessage)
                    .interaction_response_data(|d| d.set_embeds([embed]))
            })
            .await?;
        }
    }

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn myqueue(
    ctx: Context<'_>,
    #[description = "Page"]
    #[min = 1]
    page: Option<usize>,
) -> Result<(), Error> {
    let mut page = page.unwrap_or(1);
    let mut page_count = 1;
    let guild_id = ctx.guild_id().unwrap();
    let data = ctx.data();
    let queue = data.guilds.get_queue(guild_id).await;

    let queue_lock = queue.lock().await;
    let (tracklist, info) = queue_lock.user_tracklist(ctx.author().id, page - 1);
    drop(queue_lock);

    let mut embed = CreateEmbed::default();
    embed.title("Queue").description(tracklist);
    if let Some((page_ret, page_count_ret, track_count, length)) = info {
        page = page_ret + 1;
        page_count = page_count_ret;
        embed.footer(|f| {
            f.text(format!(
                "Page {}/{} | Total queue length: {} {} ({})",
                page,
                page_count,
                track_count,
                if track_count == 1 { "track" } else { "tracks" },
                utils::length_to_string(length.as_secs())
            ))
        });
    }

    let uuid = ctx.id() as usize;
    let handle = ctx
        .send(|m| {
            m.embeds.push(embed);
            if page_count > 1 {
                m.components(|c| {
                    c.create_action_row(|r| {
                        r.create_button(|b| {
                            b.style(ButtonStyle::Primary).label("<").custom_id(uuid)
                        });
                        r.create_button(|b| {
                            b.style(ButtonStyle::Primary).label(">").custom_id(uuid + 1)
                        });
                        r
                    })
                });
            }
            m
        })
        .await?;
    let msg = handle.message().await?;
    let user_id = ctx.author().id;

    while let Some(mci) = serenity::collector::CollectComponentInteraction::new(ctx)
        .author_id(user_id)
        .message_id(msg.id)
        .timeout(Duration::from_secs(30))
        .await
    {
        let choice = mci.data.custom_id.parse::<usize>().unwrap() - uuid;
        if choice == 0 {
            if page >= 2 {
                page -= 1;
            }
        } else {
            page += 1
        };

        let queue_lock = queue.lock().await;
        let (tracklist, info) = queue_lock.user_tracklist(user_id, page - 1);
        drop(queue_lock);

        let mut embed = CreateEmbed::default();
        embed.title("Queue").description(tracklist);
        if let Some((page_ret, page_count_ret, track_count, length)) = info {
            page = page_ret + 1;
            page_count = page_count_ret;
            embed.footer(|f| {
                f.text(format!(
                    "Page {}/{} | Total queue length: {} {} ({})",
                    page,
                    page_count,
                    track_count,
                    if track_count == 1 { "track" } else { "tracks" },
                    utils::length_to_string(length.as_secs())
                ))
            });

            mci.create_interaction_response(ctx, |r| {
                r.kind(InteractionResponseType::UpdateMessage)
                    .interaction_response_data(|d| d.set_embeds([embed]))
            })
            .await?;
        }
    }

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn clear(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let queue = ctx.data().guilds.get_queue(guild_id).await;
    let mut queue_lock = queue.lock().await;
    queue_lock.clear(ctx.author().id);
    ctx.say("Queue cleared").await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let (lava, queue) = utils::voice_check(&ctx, false).await?;
    if queue.lock().await.stop(lava).await.is_err() {
        return Err("Error stoping".into());
    }
    ctx.say("Done.").await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn remove(
    ctx: Context<'_>,
    #[description = "Index"]
    #[min = 1]
    index: usize,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let queue = ctx.data().guilds.get_queue(guild_id).await;
    let mut queue_lock = queue.lock().await;
    match queue_lock.remove(index - 1, ctx.author().id) {
        Some(track) => {
            ctx.send(|m| {
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

#[poise::command(slash_command, rename = "move", category = "Music")]
pub async fn mv(
    ctx: Context<'_>,
    #[description = "From"]
    #[min = 1]
    from: usize,
    #[description = "To"]
    #[min = 1]
    to: Option<usize>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let to = to.unwrap_or(1);

    let queue = ctx.data().guilds.get_queue(guild_id).await;
    let mut queue_lock = queue.lock().await;
    match queue_lock.move_track(from - 1, to - 1, ctx.author().id) {
        Some(track) => {
            ctx.send(|m| {
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

#[poise::command(slash_command, category = "Music")]
pub async fn swap(
    ctx: Context<'_>,
    #[description = "First"]
    #[min = 1]
    first: usize,
    #[description = "Second"]
    #[min = 1]
    second: usize,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let queue = ctx.data().guilds.get_queue(guild_id).await;
    let mut queue_lock = queue.lock().await;
    match queue_lock.swap(first - 1, second - 1, ctx.author().id) {
        Some((first, second)) => {
            ctx.send(|m| {
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

#[poise::command(slash_command, category = "Music")]
pub async fn skip(ctx: Context<'_>) -> Result<(), Error> {
    let (lava, queue) = utils::voice_check(&ctx, false).await?;
    queue.lock().await.skip(lava).await?;
    ctx.say("Track skipped").await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn shuffle(ctx: Context<'_>) -> Result<(), Error> {
    let (_, queue) = utils::voice_check(&ctx, false).await?;
    let mut queue_lock = queue.lock().await;
    queue_lock.shuffle(ctx.author().id);
    ctx.say("Queue shuffled").await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn seek(
    ctx: Context<'_>,
    #[description = "Time in seconds"] time: u64,
) -> Result<(), Error> {
    let position = Duration::from_secs(time);
    let guild_id = ctx.guild_id().unwrap();
    let (lava, _) = utils::voice_check(&ctx, false).await?;
    lava.seek(guild_id, position).await?;
    ctx.say("Done").await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn pause(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let (lava, _) = utils::voice_check(&ctx, false).await?;
    lava.pause(guild_id).await?;
    ctx.say("Track paused").await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn resume(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let (lava, _) = utils::voice_check(&ctx, false).await?;
    lava.resume(guild_id).await?;
    ctx.say("Track resumed").await?;

    Ok(())
}

#[poise::command(slash_command, rename = "loop", category = "Music")]
pub async fn repeat(
    ctx: Context<'_>,
    #[description = "Mode"] mode: LoopModes,
) -> Result<(), Error> {
    let (_, queue) = utils::voice_check(&ctx, false).await?;
    let mut queue_lock = queue.lock().await;
    queue_lock.set_loop_mode(mode, ctx.author().id);
    ctx.say(format!("Loop mode set to {mode}")).await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn volume(
    ctx: Context<'_>,
    #[description = "Volume"]
    #[min = 0]
    #[max = 1000]
    volume: u16,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let (lava, _) = utils::voice_check(&ctx, false).await?;
    lava.volume(guild_id, volume).await?;
    ctx.say(format!("Volume set to {volume}")).await?;

    Ok(())
}

#[poise::command(slash_command, category = "Music")]
pub async fn lyrics(ctx: Context<'_>, #[description = "Query"] query: String) -> Result<(), Error> {
    let genius = &ctx.data().genius;

    let response = genius.search(&query).await?;
    if response.is_empty() {
        return Err("Lyrics not found".into());
    }
    let url = &response[0].result.url;
    let title = &response[0].result.full_title;
    let lyrics = genius.get_lyrics(url).await?;

    ctx.send(|m| {
        m.embed(|e| {
            e.author(|a| a.name("Lyrics"))
                .title(title)
                .url(url)
                .description(lyrics.join("\n"))
        })
    })
    .await?;

    Ok(())
}
