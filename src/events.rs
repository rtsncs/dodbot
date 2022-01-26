use crate::guild::Guild;
use crate::music::queue::Queue;
use crate::shared_data::*;
use lavalink_rs::{
    gateway::LavalinkEventHandler,
    model::{Event, SendOpcode, TrackFinish, TrackStart, VoiceUpdate},
    LavalinkClient,
};
use serenity::{
    async_trait,
    http::Http,
    model::id::GuildId,
    prelude::*,
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tracing::{error, info};

pub struct Handler;
pub struct LavalinkHandler {
    pub guilds: Arc<Mutex<HashMap<GuildId, Arc<Mutex<Guild>>>>>,
    pub http: Http,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: serenity::model::prelude::Ready) {
        info!("{} connected", ready.user.name);

        {
            let data = ctx.data.read().await;
            let guilds = data.get::<Guilds>().expect("Guilds missing");
            let mut guilds_lock = guilds.lock().await;

            for guild in ready.guilds {
                let guild_id = guild.id();

                guilds_lock.insert(guild_id, Guild::new(guild_id, &ctx).await);
            }
        }

        let ctx = Arc::new(ctx);
        tokio::spawn(async move {
            loop {
                update_mc_channels(Arc::clone(&ctx)).await;
                tokio::time::sleep(Duration::from_secs(5 * 60)).await;
            }
        });
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

            let lava_inner = lava.inner.lock();
            if SendOpcode::VoiceUpdate(payload)
                .send(guild_id, lava_inner.socket_write.lock().as_mut().unwrap())
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
            let mut queue_lock = queue.lock().await;
            queue_lock.clean_up();
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
        let guild_id = GuildId(event.guild_id.0);
        let guilds_lock = self.guilds.lock().await;
        let guild = guilds_lock.get(&guild_id).unwrap();
        let guild_lock = guild.lock().await;
        let queue = guild_lock.queue.clone();
        let mut queue_lock = queue.lock().await;

        queue_lock.play_next(lava, &self.http).await;
    }
}

async fn update_mc_channels(ctx: Arc<Context>) {
    info!("Starting MC channels update");
    let data = ctx.data.read().await;
    let db = data.get::<Database>().unwrap();

    match sqlx::query!("SELECT guild_id, mc_addresses, mc_channels, mc_names FROM guilds")
        .fetch_all(db)
        .await
    {
        Ok(rows) => {
            for record in rows {
                for (i, channel_id) in record.mc_channels.iter().enumerate() {
                    if let Some(mut channel) = ctx.cache.guild_channel(*channel_id as u64).await {
                        let address = &record.mc_addresses[i];
                        let name = &record.mc_names[i];

                        let ip: Vec<&str> = address.split(':').collect();
                        let port = match ip.get(1) {
                            Some(port) => port.parse().unwrap_or(25565),
                            None => 25565,
                        };

                        let config =
                            async_minecraft_ping::ConnectionConfig::build(ip[0]).with_port(port);
                        if let Ok(mut connection) = config.connect().await {
                            if let Ok(status) = connection.status().await {
                                if let Err(why) = channel
                                    .edit(&*ctx, |c| {
                                        c.name(name.replace(
                                            '$',
                                            &format!(
                                                "{}/{}",
                                                status.players.online, status.players.max
                                            ),
                                        ))
                                    })
                                    .await
                                {
                                    error!(
                                        "Error updating MC channel {} in guild {}: {}",
                                        address, record.guild_id, why
                                    );
                                }
                            } else {
                                error!("Error getting status from {}", address);
                            }
                        } else {
                            error!("Error connecting to {}", address);
                            if let Err(why) = channel
                                .edit(&*ctx, |c| c.name(name.replace('$', "offline")))
                                .await
                            {
                                error!(
                                    "Error updating MC channel {} in guild {}: {}",
                                    address, record.guild_id, why
                                );
                            }
                        }
                    }
                }
            }
            info!("MC channels updated");
        }
        Err(why) => error!("Error fetching mc settings for all guilds: {}", why),
    }
}
