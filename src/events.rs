use crate::{
    error::Error,
    shared_data::{Data, Guilds},
};
use lavalink_rs::{
    gateway::LavalinkEventHandler,
    model::{Event, SendOpcode, TrackFinish, TrackStart, VoiceUpdate},
    LavalinkClient,
};
use serenity::{async_trait, http::Http, model::id::GuildId, prelude::*};
use std::sync::Arc;
use tracing::{error, info};

pub struct LavalinkHandler {
    pub guilds: Guilds,
    pub http: Arc<Http>,
}

pub async fn event_listener(
    ctx: &Context,
    event: &poise::Event<'_>,
    _framework: &poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    if let poise::Event::VoiceStateUpdate { old, new } = event {
        if new.user_id == ctx.cache.current_user_id() {
            if let Some(old) = old {
                if old.channel_id != new.channel_id {
                    let guild_id = new.guild_id.unwrap();
                    if new.channel_id.is_some() {
                        info!("Moved channel in guild {guild_id}");

                        let lava = &data.lavalink;
                        if lava.pause(guild_id).await.is_err() {
                            error!("Error pausing track");
                        }

                        // wait for the call to update
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        let manager = songbird::get(ctx).await.unwrap();
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

                        let socket;
                        {
                            let lava_inner = lava.inner.lock();
                            socket = lava_inner.socket_sender.read().as_ref().unwrap().clone();
                        }

                        if SendOpcode::VoiceUpdate(payload)
                            .send(guild_id, socket)
                            .await
                            .is_err()
                        {
                            error!("Error updating voice channel!");
                        }

                        if lava.resume(guild_id).await.is_err() {
                            error!("Error resuming track");
                        }
                    } else {
                        let queue = data.guilds.get_queue(guild_id).await;
                        let mut queue_lock = queue.lock().await;
                        queue_lock.clean_up();
                        let lava = &data.lavalink;
                        let _err = lava.destroy(guild_id).await;
                    }
                }
            }
        }
    }
    Ok(())
}

#[async_trait]
impl LavalinkEventHandler for LavalinkHandler {
    async fn track_start(&self, _lava: LavalinkClient, event: TrackStart) {
        info!("Track started in guild {}", event.guild_id);
    }
    async fn track_finish(&self, lava: LavalinkClient, event: TrackFinish) {
        info!("Track finished in guild {}", event.guild_id);
        let guild_id = GuildId(event.guild_id.0);
        let queue = self.guilds.get_queue(guild_id).await;
        let mut queue_lock = queue.lock().await;

        queue_lock.play_next(lava, &self.http).await;
    }
}

pub async fn update_mc_channels(ctx: Arc<serenity::prelude::Context>, database: &sqlx::PgPool) {
    info!("Starting MC channels update");

    match sqlx::query!("SELECT guild_id, mc_addresses, mc_channels, mc_names FROM guilds")
        .fetch_all(database)
        .await
    {
        Ok(rows) => {
            for record in rows {
                for (i, channel_id) in record.mc_channels.iter().enumerate() {
                    if let Some(mut channel) = ctx.cache.clone().guild_channel(*channel_id as u64) {
                        let address = &record.mc_addresses[i];
                        let name = &record.mc_names[i];

                        let ip: Vec<&str> = address.split(':').collect();
                        let port = match ip.get(1) {
                            Some(port) => port.parse().unwrap_or(25565),
                            None => 25565,
                        };

                        let config =
                            async_minecraft_ping::ConnectionConfig::build(ip[0]).with_port(port);
                        if let Ok(connection) = config.connect().await {
                            if let Ok(connection) = connection.status().await {
                                if let Err(why) = channel
                                    .edit(&*ctx, |c| {
                                        c.name(name.replace(
                                            '$',
                                            &format!(
                                                "{}/{}",
                                                connection.status.players.online,
                                                connection.status.players.max
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
