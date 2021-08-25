use crate::{music::queue::Queue, shared_data::Database, Guilds};
use serenity::{client::Context, model::id::GuildId, prelude::Mutex};
use std::sync::Arc;

pub struct Guild {
    pub guild_id: GuildId,
    pub queue: Arc<Mutex<Queue>>,
    pub prefix: String,
}
impl Guild {
    pub async fn new(guild_id: GuildId, ctx: &Context) -> Arc<Mutex<Self>> {
        let data = ctx.data.read().await;
        let db = data.get::<Database>().unwrap();
        let (prefix, round_robin) = match sqlx::query!(
            "SELECT prefix, round_robin FROM guilds WHERE guild_id = $1",
            guild_id.0 as i64
        )
        .fetch_one(db)
        .await
        {
            Ok(result) => (
                result.prefix.unwrap_or_else(|| "!".to_string()),
                result.round_robin,
            ),
            Err(_) => ("!".to_string(), false),
        };

        Arc::new(Mutex::new(Guild {
            guild_id,
            queue: Queue::new(guild_id, None, round_robin),
            prefix,
        }))
    }
    pub async fn get(ctx: &Context, guild_id: GuildId) -> Arc<Mutex<Self>> {
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
