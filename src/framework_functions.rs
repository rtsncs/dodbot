use crate::Context;

pub fn before(ctx: Context) {
    let guild_name = match ctx.guild() {
        Some(guild) => guild.name,
        None => "Direct Message".to_string(),
    };
    tracing::info!(
        "Got command '{}' by user '{}' in guild '{}'",
        ctx.command().name,
        ctx.author().name,
        guild_name
    );
}
