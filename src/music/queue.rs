use crate::guild::Guild;
use parking_lot::Mutex;
use serenity::{
    async_trait,
    client::Context,
    http::Http,
    model::id::{ChannelId, GuildId},
};
use songbird::{
    tracks::{Track, TrackHandle},
    Call, EventHandler,
};
use std::{collections::VecDeque, sync::Arc};

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
                    println!("Error playing track");
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
                println!("Error sending message: {:?}", why);
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
    pub async fn get(ctx: &Context, guild_id: &GuildId) -> Arc<Self> {
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
        let mut tracks = self.tracks.lock();
        tracks.clear();
    }

    pub fn stop(&self) {
        let mut tracks = self.tracks.lock();

        for track in tracks.drain(..) {
            let _ = track.stop();
        }
    }
}
