use serenity::{
    client::Context,
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
};
use songbird::input::Restartable;

use super::utils;

#[command]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let channel_id = match channel_id {
        Some(channel_id) => channel_id,
        None => {
            msg.reply(&ctx.http, "You must be in voice channel").await?;
            return Ok(());
        }
    };

    match utils::join(ctx, guild_id, channel_id).await {
        Ok(_) => {
            utils::react_ok(ctx, msg).await;
        }
        Err(_) => {
            msg.reply(ctx, "Error joining the voice channel").await?;
        }
    }

    Ok(())
}

#[command]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Error: songbird client missing")
        .clone();

    let has_handler = manager.get(guild_id).is_some();
    let bot_id = ctx
        .http
        .get_current_user()
        .await
        .expect("Error accessing bot id")
        .id;

    if has_handler {
        manager.remove(guild_id).await?;
        utils::react_ok(ctx, msg).await;
    } else if guild.voice_states.get(&bot_id).is_some() {
        guild
            .member(ctx, bot_id)
            .await
            .unwrap()
            .disconnect_from_voice(ctx)
            .await?;

        utils::react_ok(ctx, msg).await;
    } else {
        msg.reply(ctx, "Not in voice chat").await?;
    }

    Ok(())
}

#[command]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.message();

    let is_url = query.starts_with("http");

    match utils::voice_check(ctx, msg).await {
        Ok(handler_lock) => {
            let mut handler = handler_lock.lock().await;

            let source = match Restartable::ytdl_search(query, true).await {
                Ok(source) => source,
                Err(why) => {
                    msg.reply(&ctx.http, format!("{:?}", why)).await?;
                    return Ok(());
                }
            };

            let track = handler.play_only_source(source.into());
            let title = &track.metadata().title;

            if is_url || title.is_none() {
                utils::react_ok(ctx, msg).await;
            } else {
                msg.reply(
                    &ctx.http,
                    format!("{} is now playing", title.clone().unwrap()),
                )
                .await?;
            }
        }
        Err(why) => {
            msg.reply(&ctx.http, why).await?;
        }
    }

    return Ok(());
}
