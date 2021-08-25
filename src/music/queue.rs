use crate::guild::Guild;
use lavalink_rs::{
    model::{LavalinkResult, Track},
    LavalinkClient,
};
use rand::prelude::SliceRandom;
use serenity::{
    client::Context,
    http::Http,
    model::id::{ChannelId, GuildId, UserId},
    prelude::Mutex,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};
use tracing::error;

#[derive(Clone)]
pub struct QueuedTrack {
    pub query: String,
    pub title: String,
    pub artist: String,
    pub length: Duration,
    pub lava_track: Option<Track>,
    pub requester: UserId,
}
impl QueuedTrack {
    pub fn new(query: String, artist: String, length: Duration, requester: UserId) -> Self {
        QueuedTrack {
            title: query.clone(),
            query,
            artist,
            length,
            lava_track: None,
            requester,
        }
    }

    pub fn new_initialized(lava_track: Track, requester: UserId) -> Self {
        let info = lava_track.info.clone().unwrap();
        QueuedTrack {
            query: info.uri,
            title: info.title,
            artist: info.author,
            length: Duration::from_millis(info.length),
            lava_track: Some(lava_track),
            requester,
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
    pub channel_id: Option<ChannelId>,
    loop_mode: LoopModes,
    skipped: bool,
    tracks: VecDeque<QueuedTrack>,
    current_track: Option<QueuedTrack>,
    round_robin: bool,
    users: VecDeque<UserId>,
    user_tracks: HashMap<UserId, VecDeque<QueuedTrack>>,
}
impl Queue {
    pub fn new(
        guild_id: GuildId,
        channel_id: Option<ChannelId>,
        round_robin: bool,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Queue {
            guild_id,
            channel_id,
            loop_mode: LoopModes::None,
            skipped: false,
            tracks: VecDeque::default(),
            current_track: None,
            round_robin,
            users: VecDeque::default(),
            user_tracks: HashMap::default(),
        }))
    }
    pub async fn get(ctx: &Context, guild_id: GuildId) -> Arc<Mutex<Self>> {
        let guild = Guild::get(ctx, guild_id).await;
        let guild_lock = guild.lock().await;

        guild_lock.queue.clone()
    }

    pub async fn enqueue(
        &mut self,
        mut track: QueuedTrack,
        lava: LavalinkClient,
    ) -> Result<(), ()> {
        if self.tracks.is_empty() {
            if let Ok(lava_track) = track.init(&lava).await {
                if lava.play(self.guild_id, lava_track).queue().await.is_err() {
                    return Err(());
                }
                self.current_track = Some(track);
            } else {
                return Err(());
            }
        } else {
            self.tracks.push_back(track);
        }

        Ok(())
    }

    pub async fn enqueue_multiple(
        &mut self,
        mut tracks: Vec<QueuedTrack>,
        lava: LavalinkClient,
    ) -> Result<(), ()> {
        if self.tracks.is_empty() {
            let mut track = tracks.remove(0);
            if let Ok(lava_track) = track.init(&lava).await {
                if lava.play(self.guild_id, lava_track).queue().await.is_err() {
                    return Err(());
                };
                self.current_track = Some(track);
            } else {
                return Err(());
            }
        }

        self.tracks.append(&mut tracks.into());
        Ok(())
    }

    pub fn tracklist(&self) -> VecDeque<QueuedTrack> {
        self.tracks.clone()
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
    }

    pub async fn stop(&mut self, lava: LavalinkClient) -> LavalinkResult<()> {
        self.skipped = true;
        self.tracks.clear();

        lava.skip(self.guild_id).await;
        lava.stop(self.guild_id).await
    }

    pub fn remove(&mut self, index: usize) -> Option<QueuedTrack> {
        self.tracks.remove(index)
    }

    pub fn move_track(&mut self, from: usize, to: usize) -> Option<QueuedTrack> {
        let track = self.tracks.remove(from)?;
        let handle = track.clone();
        self.tracks.insert(to, track);

        Some(handle)
    }

    pub fn swap(&mut self, first: usize, second: usize) -> Option<(QueuedTrack, QueuedTrack)> {
        let len = self.tracks.len();

        if !(0..len).contains(&first) || !(0..len).contains(&second) {
            return None;
        }
        let handles = (self.tracks[first].clone(), self.tracks[second].clone());
        self.tracks.swap(first, second);

        Some(handles)
    }

    pub fn shuffle(&mut self) {
        if self.tracks.len() > 1 {
            let mut old_tracks: Vec<QueuedTrack> = self.tracks.clone().into();
            self.tracks.clear();
            let mut rng = rand::thread_rng();
            old_tracks.shuffle(&mut rng);
            self.tracks.append(&mut old_tracks.into());
        }
    }

    pub async fn skip(&mut self, lava: LavalinkClient) -> LavalinkResult<()> {
        self.skipped = true;
        lava.skip(self.guild_id).await;
        lava.stop(self.guild_id).await?;

        Ok(())
    }

    pub async fn play_next(&mut self, lava: LavalinkClient, http: &Http) {
        //TODO: send message when there's an error playing a track
        if self.loop_mode == LoopModes::Song && !self.skipped {
            if lava
                .play(
                    self.guild_id,
                    self.current_track
                        .clone()
                        .unwrap()
                        .lava_track
                        .unwrap()
                        .clone(),
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

        let old = self.current_track.take();
        if self.loop_mode == LoopModes::Queue && !self.skipped {
            if let Some(old) = old {
                self.tracks.push_back(old);
            }
        }

        while let Some(mut track) = self.tracks.pop_front() {
            if let Ok(lava_track) = track.init(&lava).await {
                title = Some(track.title.clone());
                if lava.play(self.guild_id, lava_track).queue().await.is_ok() {
                    self.current_track = Some(track);
                    break;
                }
            }
            error!("Error playing track");
        }

        if let Some(title) = title {
            if let Some(channel) = self.channel_id {
                if let Err(why) = channel.say(http, format!("Now playing: {}", title)).await {
                    error!("Error sending message: {:?}", why);
                }
            }
        }
        self.skipped = false;
    }

    pub fn set_loop_mode(&mut self, mode: LoopModes) {
        self.loop_mode = mode;
    }

    pub fn set_round_robin(&mut self, round_robin: bool) {
        self.clear();
        self.round_robin = round_robin;
    }
}
