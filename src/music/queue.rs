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
use std::{collections::VecDeque, sync::Arc};
use tracing::error;

pub struct Queue {
    guild_id: GuildId,
    pub channel_id: Mutex<Option<ChannelId>>,
    tracks: Arc<Mutex<VecDeque<Track>>>,
}
impl Queue {
    pub fn new(guild_id: GuildId, channel_id: Option<ChannelId>) -> Arc<Self> {
        Arc::new(Queue {
            guild_id,
            channel_id: Mutex::new(channel_id),
            tracks: Arc::new(Mutex::new(VecDeque::default())),
        })
    }
    pub async fn get(ctx: &Context, guild_id: GuildId) -> Arc<Self> {
        let guild = Guild::get(ctx, guild_id).await;

        guild.queue.clone()
    }

    pub async fn enqueue(&self, track: Track, lava: LavalinkClient) -> LavalinkResult<()> {
        let mut tracks = self.tracks.lock().await;

        if tracks.is_empty() {
            lava.play(self.guild_id, track.clone()).queue().await?;
        }

        tracks.push_back(track);
        Ok(())
    }

    pub async fn enqueue_multiple(
        &self,
        tracks: Vec<Track>,
        lava: LavalinkClient,
    ) -> LavalinkResult<()> {
        let mut queued_tracks = self.tracks.lock().await;

        if queued_tracks.is_empty() {
            lava.play(self.guild_id, tracks[0].clone()).queue().await?;
        }

        queued_tracks.append(&mut tracks.into());
        Ok(())
    }

    pub async fn current(&self) -> Option<Track> {
        let tracks = self.tracks.lock().await;

        tracks.get(0).cloned()
    }

    pub async fn tracklist(&self) -> VecDeque<Track> {
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
        let mut tracks = self.tracks.lock().await;
        tracks.clear();

        lava.stop(self.guild_id).await
    }

    pub async fn remove(&self, index: usize) -> Option<Track> {
        let mut tracks = self.tracks.lock().await;
        if index < 1 {
            return None;
        }
        tracks.remove(index)
    }

    pub async fn move_track(&self, from: usize, to: usize) -> Option<Track> {
        let mut tracks = self.tracks.lock().await;

        let track = tracks.remove(from)?;
        let handle = track.clone();
        tracks.insert(to, track);

        Some(handle)
    }

    pub async fn swap(&self, first: usize, second: usize) -> Option<(Track, Track)> {
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
            let mut old_tracks: Vec<Track> = old_tracks.into();
            old_tracks.shuffle(&mut rng);
            tracks.append(&mut old_tracks.into());
        }
    }

    pub async fn skip(&self, lava: LavalinkClient) -> LavalinkResult<Option<Track>> {
        let tracks = self.tracks.lock().await;
        if !tracks.is_empty() {
            lava.stop(self.guild_id).await?;
        }
        Ok(None)
    }

    pub async fn play_next(&self, lava: LavalinkClient, http: &Http) {
        let mut title = None;
        {
            let mut tracks = self.tracks.lock().await;
            tracks.pop_front();

            while let Some(track) = tracks.front() {
                if lava
                    .play(self.guild_id, track.clone())
                    .queue()
                    .await
                    .is_err()
                {
                    error!("Error playing track");
                    tracks.pop_front();
                } else {
                    title = Some(track.info.as_ref().unwrap().title.clone());
                    break;
                }
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
    }
}
