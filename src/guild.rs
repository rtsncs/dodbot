use std::sync::Arc;

use crate::{music::queue::Queue, Guilds};
use serenity::{client::Context, model::id::GuildId};

pub struct Guild {
    pub guild_id: GuildId,
    pub queue: Arc<Queue>,
}
impl Guild {
    pub fn new(guild_id: GuildId) -> Arc<Self> {
        Arc::new(Guild {
            guild_id,
            queue: Queue::new(guild_id, None),
        })
    }
    pub async fn get(ctx: &Context, guild_id: GuildId) -> Arc<Self> {
        let guilds = ctx
            .data
            .read()
            .await
            .get::<Guilds>()
            .expect("Error: guild structs missing")
            .clone();
        let guilds_lock = guilds.lock().await;
        guilds_lock.get(&guild_id).unwrap().clone()
    }
}
