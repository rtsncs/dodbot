use crate::guild::Guild;
use crate::music::queue::{LoopModes, Queue};
use crate::shared_data::*;
use lavalink_rs::{
    gateway::LavalinkEventHandler,
    model::{Event, SendOpcode, TrackFinish, TrackStart, VoiceUpdate},
    LavalinkClient,
};
use serenity::{
    async_trait,
    http::Http,
    model::{guild::GuildStatus, id::GuildId},
    prelude::*,
};
use std::{collections::HashMap, sync::Arc};
use tracing::{error, info};

pub struct Handler;
pub struct LavalinkHandler {
    pub guilds: Arc<Mutex<HashMap<GuildId, Arc<Guild>>>>,
    pub http: Http,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: serenity::model::prelude::Ready) {
        info!("{} connected", ready.user.name);

        let data = ctx.data.read().await;
        let guilds = data.get::<Guilds>().expect("Guilds missing");
        let mut guilds_lock = guilds.lock().await;

        for guild in ready.guilds {
            let guild_id = match guild {
                GuildStatus::OnlineGuild(g) => g.id,
                GuildStatus::OnlinePartialGuild(g) => g.id,
                GuildStatus::Offline(g) => g.id,
                _ => continue,
            };

            guilds_lock.insert(guild_id, Guild::new(guild_id));
        }
    }

    async fn voice_state_update(
        &self,
        ctx: Context,
        guild_id: Option<GuildId>,
        old: Option<serenity::model::prelude::VoiceState>,
        new: serenity::model::prelude::VoiceState,
    ) {
        if new.user_id != ctx.cache.current_user_id().await {
            return;
        }
        match &old {
            Some(old) => {
                if old.channel_id == new.channel_id {
                    return;
                }
            }
            _ => return,
        }
        if new.channel_id.is_some() {
            let guild_id = guild_id.unwrap();
            let guild_name = guild_id
                .name(&ctx)
                .await
                .unwrap_or_else(|| guild_id.to_string());
            info!("Moved channel in guild {}", guild_name);

            let data = ctx.data.read().await;
            let lava = data.get::<Lavalink>().unwrap();
            if lava.pause(guild_id).await.is_err() {
                error!("Error pausing track");
            }

            // wait for the call to update
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let manager = songbird::get(&ctx).await.unwrap();
            let call = manager.get(guild_id).unwrap();
            let call_lock = call.lock().await;
            let info = call_lock.current_connection().unwrap().clone();

            let event = Event {
                token: info.token,
                endpoint: info.endpoint,
                guild_id: info.guild_id.to_string(),
            };
            let payload = VoiceUpdate {
                session_id: info.session_id,
                event,
            };

            let mut lava_inner = lava.inner.lock().await;
            if SendOpcode::VoiceUpdate(payload)
                .send(guild_id, &mut lava_inner.socket_write)
                .await
                .is_err()
            {
                error!("Error updating voice channel!");
            }
            drop(lava_inner);

            if lava.resume(guild_id).await.is_err() {
                error!("Error resuming track");
            }
        } else {
            let queue = Queue::get(&ctx, guild_id.unwrap()).await;
            queue.clear().await;
            queue.set_loop_mode(LoopModes::None).await;
            let data = ctx.data.read().await;
            let lava = data.get::<Lavalink>().unwrap();
            let _err = lava.destroy(guild_id.unwrap()).await;
        }
    }
}

#[async_trait]
impl LavalinkEventHandler for LavalinkHandler {
    async fn track_start(&self, _lava: LavalinkClient, event: TrackStart) {
        info!("Track started in guild {}", event.guild_id);
    }
    async fn track_finish(&self, lava: LavalinkClient, event: TrackFinish) {
        info!("Track finished in guild {}", event.guild_id);
        let guild_id = GuildId(event.guild_id);
        let guilds = self.guilds.lock().await;
        let guild = guilds.get(&guild_id).unwrap();
        let queue = guild.queue.clone();

        queue.play_next(lava, &self.http).await;
    }
}
