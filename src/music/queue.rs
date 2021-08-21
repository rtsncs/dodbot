use crate::guild::Guild;
use lavalink_rs::{
    model::{LavalinkResult, Track},
    LavalinkClient,
};
use rand::prelude::SliceRandom;
use serenity::{
    client::Context,
    http::Http,
    model::id::{ChannelId, GuildId},
    prelude::Mutex,
};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tracing::error;

#[derive(Clone)]
pub struct QueuedTrack {
    pub query: String,
    pub title: String,
    pub artist: String,
    pub length: Duration,
    pub lava_track: Option<Track>,
}
impl QueuedTrack {
    pub fn new(query: String, artist: String, length: Duration) -> Self {
        QueuedTrack {
            title: query.clone(),
            query,
            artist,
            length,
            lava_track: None,
        }
    }

    pub fn new_initialized(lava_track: Track) -> Self {
        let info = lava_track.info.clone().unwrap();
        QueuedTrack {
            query: info.uri,
            title: info.title,
            artist: info.author,
            length: Duration::from_millis(info.length),
            lava_track: Some(lava_track),
        }
    }

    pub async fn init(&mut self, lava: &LavalinkClient) -> Result<Track, ()> {
        match &self.lava_track {
            Some(track) => Ok(track.clone()),
            None => {
                let query_result = lava.auto_search_tracks(&self.query).await;
                if let Ok(query_result) = query_result {
                    if query_result.tracks.is_empty() {
                        return Err(());
                    }
                    let track = query_result.tracks[0].clone();
                    let info = track.info.clone().unwrap();
                    self.query = info.uri;
                    self.title = info.title;
                    self.artist = info.author;
                    self.length = Duration::from_millis(info.length);
                    self.lava_track = Some(track.clone());
                    Ok(track)
                } else {
                    Err(())
                }
            }
        }
    }
}

#[derive(PartialEq)]
pub enum LoopModes {
    None,
    Song,
    Queue,
}

pub struct Queue {
    guild_id: GuildId,
    pub channel_id: Mutex<Option<ChannelId>>,
    loop_mode: Mutex<LoopModes>,
    skipped: Mutex<bool>,
    tracks: Arc<Mutex<VecDeque<QueuedTrack>>>,
}
impl Queue {
    pub fn new(guild_id: GuildId, channel_id: Option<ChannelId>) -> Arc<Self> {
        Arc::new(Queue {
            guild_id,
            channel_id: Mutex::new(channel_id),
            loop_mode: Mutex::new(LoopModes::None),
            skipped: Mutex::new(false),
            tracks: Arc::new(Mutex::new(VecDeque::default())),
        })
    }
    pub async fn get(ctx: &Context, guild_id: GuildId) -> Arc<Self> {
        let guild = Guild::get(ctx, guild_id).await;

        guild.queue.clone()
    }

    pub async fn enqueue(&self, mut track: QueuedTrack, lava: LavalinkClient) -> Result<(), ()> {
        let mut tracks = self.tracks.lock().await;

        if tracks.is_empty() {
            if let Ok(lava_track) = track.init(&lava).await {
                if lava.play(self.guild_id, lava_track).queue().await.is_err() {
                    return Err(());
                };
            } else {
                return Err(());
            }
        }

        tracks.push_back(track);
        Ok(())
    }

    pub async fn enqueue_multiple(
        &self,
        mut tracks: Vec<QueuedTrack>,
        lava: LavalinkClient,
    ) -> Result<(), ()> {
        let mut queued_tracks = self.tracks.lock().await;

        if queued_tracks.is_empty() {
            if let Ok(lava_track) = tracks[0].init(&lava).await {
                if lava.play(self.guild_id, lava_track).queue().await.is_err() {
                    return Err(());
                };
            } else {
                return Err(());
            }
        }

        queued_tracks.append(&mut tracks.into());
        Ok(())
    }

    pub async fn current(&self) -> Option<QueuedTrack> {
        let tracks = self.tracks.lock().await;

        tracks.get(0).cloned()
    }

    pub async fn tracklist(&self) -> VecDeque<QueuedTrack> {
        self.tracks.lock().await.clone()
    }

    pub async fn clear(&self) {
        if let Some(track) = self.current().await {
            let mut tracks = self.tracks.lock().await;
            tracks.clear();
            tracks.push_back(track);
        }
    }

    pub async fn stop(&self, lava: LavalinkClient) -> LavalinkResult<()> {
        let mut skipped = self.skipped.lock().await;
        *skipped = true;
        let mut tracks = self.tracks.lock().await;
        tracks.clear();

        lava.skip(self.guild_id).await;
        lava.stop(self.guild_id).await
    }

    pub async fn remove(&self, index: usize) -> Option<QueuedTrack> {
        let mut tracks = self.tracks.lock().await;
        if index < 1 {
            return None;
        }
        tracks.remove(index)
    }

    pub async fn move_track(&self, from: usize, to: usize) -> Option<QueuedTrack> {
        let mut tracks = self.tracks.lock().await;

        let track = tracks.remove(from)?;
        let handle = track.clone();
        tracks.insert(to, track);

        Some(handle)
    }

    pub async fn swap(&self, first: usize, second: usize) -> Option<(QueuedTrack, QueuedTrack)> {
        let mut tracks = self.tracks.lock().await;
        let len = tracks.len();

        if !(1..len).contains(&first) || !(1..len).contains(&second) {
            return None;
        }
        let handles = (tracks[first].clone(), tracks[second].clone());
        tracks.swap(first, second);

        Some(handles)
    }

    pub async fn shuffle(&self) {
        let mut tracks = self.tracks.lock().await;
        if tracks.len() > 1 {
            let mut old_tracks = tracks.clone();
            tracks.clear();
            tracks.push_back(old_tracks.pop_front().unwrap());
            let mut rng = rand::thread_rng();
            let mut old_tracks: Vec<QueuedTrack> = old_tracks.into();
            old_tracks.shuffle(&mut rng);
            tracks.append(&mut old_tracks.into());
        }
    }

    pub async fn skip(&self, lava: LavalinkClient) -> LavalinkResult<()> {
        let mut skipped = self.skipped.lock().await;
        *skipped = true;
        lava.skip(self.guild_id).await;
        lava.stop(self.guild_id).await?;

        Ok(())
    }

    pub async fn play_next(&self, lava: LavalinkClient, http: &Http) {
        //TODO: send message when there's an error playing a track
        let loop_mode = self.loop_mode.lock().await;
        let mut tracks = self.tracks.lock().await;
        let mut skipped = self.skipped.lock().await;
        if *loop_mode == LoopModes::Song && !*skipped {
            if lava
                .play(
                    self.guild_id,
                    tracks[0].lava_track.as_ref().unwrap().clone(),
                )
                .queue()
                .await
                .is_ok()
            {
                return;
            }
            error!("Error playing track!");
        }

        let mut title = None;
        {
            let old = tracks.pop_front();
            if *loop_mode == LoopModes::Queue && !*skipped {
                if let Some(old) = old {
                    tracks.push_back(old);
                }
            }

            while let Some(track) = tracks.front_mut() {
                if let Ok(lava_track) = track.init(&lava).await {
                    title = Some(track.title.clone());
                    if lava.play(self.guild_id, lava_track).queue().await.is_ok() {
                        break;
                    }
                }
                error!("Error playing track");
                tracks.pop_front();
            }
        }
        if let Some(title) = title {
            let channel_id = self.channel_id.lock().await;
            if let Some(channel) = *channel_id {
                if let Err(why) = channel.say(http, format!("Now playing: {}", title)).await {
                    error!("Error sending message: {:?}", why);
                }
            }
        }
        *skipped = false;
    }

    pub async fn set_loop_mode(&self, mode: LoopModes) {
        let mut loop_mode = self.loop_mode.lock().await;
        *loop_mode = mode;
    }
}
