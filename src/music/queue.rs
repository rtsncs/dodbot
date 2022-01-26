use crate::guild::Guild;
use lavalink_rs::{error::LavalinkResult, model::Track, LavalinkClient};
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

struct UserQueue {
    tracks: VecDeque<QueuedTrack>,
    loop_mode: LoopModes,
}
impl UserQueue {
    fn new() -> Self {
        UserQueue {
            tracks: VecDeque::default(),
            loop_mode: LoopModes::None,
        }
    }
}

pub struct Queue {
    guild_id: GuildId,
    pub channel_id: Option<ChannelId>,
    loop_mode: LoopModes,
    skipped: bool,
    tracks: VecDeque<QueuedTrack>,
    pub current_track: Option<QueuedTrack>,
    round_robin: bool,
    users: VecDeque<UserId>,
    user_queues: HashMap<UserId, UserQueue>,
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
            user_queues: HashMap::default(),
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
        if self.current_track.is_none() {
            if let Ok(lava_track) = track.init(&lava).await {
                if lava.play(self.guild_id, lava_track).queue().await.is_err() {
                    return Err(());
                }
                self.current_track = Some(track);
            } else {
                return Err(());
            }
        } else if self.round_robin {
            let user = track.requester;
            let queue = self.user_queues.get_mut(&user);
            if let Some(queue) = queue {
                queue.tracks.push_back(track);
            } else {
                let mut queue = UserQueue::new();
                queue.tracks.push_back(track);
                self.user_queues.insert(user, queue);
                self.users.push_back(user);
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
        if self.current_track.is_none() {
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
        if self.round_robin {
            let user = tracks[0].requester;
            let queue = self.user_queues.get_mut(&user);
            if let Some(queue) = queue {
                queue.tracks.append(&mut tracks.into());
            } else {
                let mut queue = UserQueue::new();
                queue.tracks.append(&mut tracks.into());
                self.user_queues.insert(user, queue);
                self.users.push_back(user);
            }
        } else {
            self.tracks.append(&mut tracks.into());
        }

        Ok(())
    }

    pub fn tracklist(&self, mut page: usize) -> (String, Option<(usize, usize, usize, Duration)>) {
        let mut tracklist = String::new();
        let mut info = None;
        if self.round_robin {
            if self.users.is_empty() {
                tracklist += "The queue is empty.";
            } else {
                let mut len = 0;
                for queue in self.user_queues.values() {
                    len += queue.tracks.len();
                }
                let page_count = (len as f32 / 20.).ceil() as usize;
                if page > page_count - 1 {
                    page = page_count - 1;
                }
                let mut length = Duration::new(0, 0);

                let mut users = self.users.clone();
                let cur_user = users.pop_front().unwrap();
                users.push_back(cur_user);

                let mut track_num = 1;
                let mut user_index = 0;
                let mut user_tracks = vec![0; users.len()];
                while track_num <= len {
                    let user = users[user_index];
                    let queue = self.user_queues.get(&user).unwrap();
                    if user_tracks[user_index] < queue.tracks.len() {
                        let track = &queue.tracks[user_tracks[user_index]];
                        if track_num > page * 20 && track_num <= page * 20 + 20 {
                            let title = &track.title;
                            let duration =
                                crate::music::utils::length_to_string(track.length.as_secs());
                            let requester = track.requester.0;

                            tracklist += &format!(
                                "{}. {} ({}) - <@{}>\n",
                                track_num, title, duration, requester
                            );
                        }
                        length += track.length;
                        track_num += 1;
                        user_tracks[user_index] += 1;
                    }
                    user_index += 1;
                    if user_index > users.len() - 1 {
                        user_index = 0;
                    }
                }
                info = Some((page, page_count, track_num - 1, length));
            }
        } else if self.tracks.is_empty() {
            tracklist += "The queue is empty.";
        } else {
            let page_count = (self.tracks.len() as f32 / 20.).ceil() as usize;
            if page > page_count - 1 {
                page = page_count - 1;
            }
            let mut length = Duration::new(0, 0);
            for (i, track) in self.tracks.iter().enumerate() {
                length += track.length;
                if i >= page * 20 && i < page * 20 + 20 {
                    let title = &track.title;
                    let duration = crate::music::utils::length_to_string(track.length.as_secs());
                    let requester = track.requester.0;
                    tracklist +=
                        &format!("{}. {} ({}) - <@{}>\n", i + 1, title, duration, requester);
                }
            }
            info = Some((page, page_count, self.tracks.len(), length));
        }
        (tracklist, info)
    }

    pub fn user_tracklist(
        &self,
        user: UserId,
        mut page: usize,
    ) -> (String, Option<(usize, usize, usize, Duration)>) {
        let mut tracklist = String::new();
        let mut info = None;
        if !self.round_robin {
            tracklist += "Round robin is disabled on this server.";
        } else if let Some(queue) = self.user_queues.get(&user) {
            let page_count = (queue.tracks.len() as f32 / 20.).ceil() as usize;
            if page > page_count - 1 {
                page = page_count - 1;
            }
            let mut length = Duration::new(0, 0);
            for (i, track) in queue.tracks.iter().enumerate() {
                length += track.length;
                if i >= page * 20 && i < page * 20 + 20 {
                    let title = &track.title;
                    let duration = crate::music::utils::length_to_string(track.length.as_secs());
                    tracklist += &format!("{}. {} ({})\n", i + 1, title, duration);
                }
            }
            info = Some((page, page_count, queue.tracks.len(), length));
        } else {
            tracklist += "Your queue is empty.";
        }
        (tracklist, info)
    }

    pub fn clear(&mut self, user: UserId) {
        if self.round_robin && self.user_queues.remove(&user).is_some() {
            if let Ok(index) = self.users.binary_search(&user) {
                self.users.remove(index);
            }
        } else {
            self.tracks.clear();
        }
    }

    pub async fn stop(&mut self, lava: LavalinkClient) -> LavalinkResult<()> {
        self.skipped = true;
        if self.round_robin {
            self.users.clear();
            self.user_queues.clear();
        } else {
            self.tracks.clear();
        }

        lava.skip(self.guild_id).await;
        lava.stop(self.guild_id).await
    }

    pub fn remove(&mut self, index: usize, user: UserId) -> Option<QueuedTrack> {
        if self.round_robin {
            if let Some(queue) = self.user_queues.get_mut(&user) {
                let track = queue.tracks.remove(index);
                if queue.tracks.is_empty() {
                    self.user_queues.remove(&user);
                    if let Ok(index) = self.users.binary_search(&user) {
                        self.users.remove(index);
                    }
                }

                return track;
            }
            None
        } else {
            self.tracks.remove(index)
        }
    }

    pub fn move_track(&mut self, from: usize, to: usize, user: UserId) -> Option<QueuedTrack> {
        let handle = if self.round_robin {
            let queue = self.user_queues.get_mut(&user)?;
            let track = queue.tracks.remove(from)?;
            queue.tracks.insert(to, track.clone());
            track
        } else {
            let track = self.tracks.remove(from)?;
            self.tracks.insert(to, track.clone());
            track
        };

        Some(handle)
    }

    pub fn swap(
        &mut self,
        first: usize,
        second: usize,
        user: UserId,
    ) -> Option<(QueuedTrack, QueuedTrack)> {
        if self.round_robin {
            match self.user_queues.get_mut(&user) {
                Some(queue) => {
                    let len = queue.tracks.len();

                    if !(0..len).contains(&first) || !(0..len).contains(&second) {
                        return None;
                    }
                    let handles = (queue.tracks[first].clone(), queue.tracks[second].clone());
                    queue.tracks.swap(first, second);

                    return Some(handles);
                }
                None => return None,
            }
        }
        let len = self.tracks.len();

        if !(0..len).contains(&first) || !(0..len).contains(&second) {
            return None;
        }
        let handles = (self.tracks[first].clone(), self.tracks[second].clone());
        self.tracks.swap(first, second);

        Some(handles)
    }

    pub fn shuffle(&mut self, user: UserId) {
        if self.round_robin {
            if let Some(queue) = self.user_queues.get_mut(&user) {
                if queue.tracks.len() > 1 {
                    let mut old_tracks: Vec<QueuedTrack> = queue.tracks.clone().into();
                    queue.tracks.clear();
                    let mut rng = rand::thread_rng();
                    old_tracks.shuffle(&mut rng);
                    queue.tracks.append(&mut old_tracks.into());
                }
            }
        } else if self.tracks.len() > 1 {
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
        let mut title = None;
        if self.round_robin {
            let prev_track = self.current_track.take();
            if let Some(prev_track) = prev_track {
                if let Some(prev_user) = self.users.pop_front() {
                    if let Some(queue) = self.user_queues.get_mut(&prev_user) {
                        if !self.skipped {
                            if queue.loop_mode == LoopModes::Song {
                                queue.tracks.push_front(prev_track);
                            } else if queue.loop_mode == LoopModes::Queue {
                                queue.tracks.push_back(prev_track);
                            }
                        }
                        if queue.tracks.is_empty() {
                            self.user_queues.remove(&prev_user);
                        } else {
                            self.users.push_back(prev_user);
                        }
                    }
                }
            }
            while let Some(next_user) = self.users.front() {
                let queue = self.user_queues.get_mut(next_user).unwrap();
                if let Some(mut track) = queue.tracks.pop_front() {
                    if let Ok(lava_track) = track.init(&lava).await {
                        title = Some(track.title.clone());
                        if lava.play(self.guild_id, lava_track).queue().await.is_ok() {
                            self.current_track = Some(track);
                            break;
                        }
                    }
                    error!("Error playing track");
                } else {
                    self.users.pop_front();
                }
            }
        } else {
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

            let prev_track = self.current_track.take();
            if self.loop_mode == LoopModes::Queue && !self.skipped {
                if let Some(old) = prev_track {
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
        }
        if let Some(channel) = self.channel_id {
            if let Some(title) = title {
                if let Err(why) = channel
                    .send_message(http, |m| {
                        m.embed(|e| e.title("Now playing").description(title))
                    })
                    .await
                {
                    error!("Error sending message: {:?}", why);
                }
            } else if let Err(why) = channel
                .send_message(http, |m| m.embed(|e| e.description("The queue has ended")))
                .await
            {
                error!("Error sending message: {:?}", why);
            }
        }
        self.skipped = false;
    }

    pub fn set_loop_mode(&mut self, mode: LoopModes, user: UserId) {
        if self.round_robin {
            if let Some(queue) = self.user_queues.get_mut(&user) {
                if let Some(track) = &self.current_track {
                    if track.requester == user && mode != LoopModes::None {
                        queue.tracks.push_back(track.clone());
                    }
                }
                queue.loop_mode = mode;
            } else if let Some(track) = &self.current_track {
                if mode != LoopModes::None && track.requester == user {
                    let mut queue = UserQueue::new();
                    queue.tracks.push_back(track.clone());
                    queue.loop_mode = mode;
                    self.user_queues.insert(user, queue);
                    self.users.push_back(user);
                }
            }
        } else {
            self.loop_mode = mode;
        }
    }

    pub fn set_round_robin(&mut self, round_robin: bool) {
        if self.round_robin && !round_robin {
            self.users.clear();
            self.user_queues.clear();
        } else if !self.round_robin && round_robin {
            self.tracks.clear();
        }
        self.round_robin = round_robin;
    }

    pub fn clean_up(&mut self) {
        self.tracks.clear();
        self.loop_mode = LoopModes::None;
        self.users.clear();
        self.user_queues.clear();
    }
}
