use crate::guild::Guild;
use parking_lot::Mutex;
use rand::prelude::SliceRandom;
use serenity::{
    async_trait,
    client::Context,
    http::Http,
    model::id::{ChannelId, GuildId},
};
use songbird::{
    tracks::{Track, TrackHandle, TrackResult},
    Call, EventHandler,
};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tracing::error;

pub struct TrackEnd {
    pub queue: Arc<Queue>,
    pub channel_id: ChannelId,
    pub http: Arc<Http>,
}

#[async_trait]
impl EventHandler for TrackEnd {
    async fn act(&self, _ctx: &songbird::EventContext<'_>) -> Option<songbird::Event> {
        let mut title = None;
        {
            let mut tracks = self.queue.tracks.lock();
            tracks.pop_front();

            while let Some(track) = tracks.front() {
                if track.play().is_err() {
                    error!("Error playing track");
                    tracks.pop_front();
                } else {
                    title = track.metadata().title.clone();
                    break;
                }
            }
        }
        if let Some(title) = title {
            if let Err(why) = self
                .channel_id
                .say(self.http.clone(), format!("Now playing: {}", title))
                .await
            {
                error!("Error sending message: {:?}", why);
            }
        }
        None
    }
}

pub struct Queue {
    tracks: Arc<Mutex<VecDeque<TrackHandle>>>,
}
impl Queue {
    pub fn new() -> Arc<Self> {
        Arc::new(Queue {
            tracks: Arc::new(Mutex::new(VecDeque::default())),
        })
    }
    pub async fn get(ctx: &Context, guild_id: GuildId) -> Arc<Self> {
        let guild = Guild::get(ctx, guild_id).await;

        guild.queue.clone()
    }

    pub fn enqueue(&self, mut track: Track, mut handler: tokio::sync::MutexGuard<'_, Call>) {
        let mut tracks = self.tracks.lock();

        let handle = track.handle.clone();

        if !tracks.is_empty() {
            track.pause();
        }
        tracks.push_back(handle);

        handler.play(track);
    }

    pub fn current(&self) -> Option<TrackHandle> {
        let tracks = self.tracks.lock();

        tracks.get(0).cloned()
    }

    pub fn tracklist(&self) -> VecDeque<TrackHandle> {
        self.tracks.lock().clone()
    }

    pub fn clear(&self) {
        if let Some(track) = self.current() {
            let mut tracks = self.tracks.lock();
            tracks.clear();
            tracks.push_back(track);
        }
    }

    pub fn stop(&self) {
        let mut tracks = self.tracks.lock();

        for track in tracks.drain(..) {
            let _ = track.stop();
        }
    }

    pub fn remove(&self, index: usize) -> Option<TrackHandle> {
        let mut tracks = self.tracks.lock();
        if index < 1 {
            return None;
        }
        tracks.remove(index)
    }

    pub fn move_track(&self, from: usize, to: usize) -> Option<TrackHandle> {
        let mut tracks = self.tracks.lock();

        let track = tracks.remove(from)?;
        let handle = track.clone();
        tracks.insert(to, track);

        Some(handle)
    }

    pub fn swap(&self, first: usize, second: usize) -> Option<(TrackHandle, TrackHandle)> {
        let mut tracks = self.tracks.lock();
        let len = tracks.len();

        if !(1..len).contains(&first) || !(1..len).contains(&second) {
            return None;
        }
        let handles = (tracks[first].clone(), tracks[second].clone());
        tracks.swap(first, second);

        Some(handles)
    }

    pub fn shuffle(&self) {
        let mut tracks = self.tracks.lock();
        if tracks.len() > 1 {
            let mut old_tracks = tracks.clone();
            tracks.clear();
            tracks.push_back(old_tracks.pop_front().unwrap());
            let mut rng = rand::thread_rng();
            let mut old_tracks: Vec<TrackHandle> = old_tracks.into();
            old_tracks.shuffle(&mut rng);
            tracks.append(&mut old_tracks.into());
        }
    }

    pub fn skip(&self) {
        if let Some(track) = self.current() {
            let _ = track.stop();
        }
    }

    pub fn seek(&self, position: Duration) -> TrackResult<()> {
        if let Some(track) = self.current() {
            return track.seek_time(position);
        }
        Ok(())
    }

    pub fn pause(&self) -> TrackResult<()> {
        if let Some(track) = self.current() {
            return track.pause();
        }
        Ok(())
    }

    pub fn resume(&self) -> TrackResult<()> {
        if let Some(track) = self.current() {
            return track.play();
        }
        Ok(())
    }
}
