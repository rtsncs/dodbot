use crate::music::queue::Queue;
use serenity::{model::id::GuildId, prelude::Mutex};
use sqlx::PgPool;
use std::sync::Arc;

pub struct Guild {
    pub guild_id: GuildId,
    pub queue: Arc<Mutex<Queue>>,
    pub prefix: String,
}
impl Guild {
    pub async fn new(guild_id: GuildId, database: &PgPool) -> Arc<Mutex<Self>> {
        let (prefix, round_robin) = match sqlx::query!(
            "SELECT prefix, round_robin FROM guilds WHERE guild_id = $1",
            guild_id.0 as i64
        )
        .fetch_one(database)
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
}
