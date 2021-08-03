use serenity::{
    client::Context,
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
};

#[command]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            msg.reply(&ctx.http, "Not in vc").await?;
            return Ok(());
        }
    };

    let manager = songbird::get(ctx)
        .await
        .expect("Error: songbird client missing")
        .clone();

    let _handler = manager.join(guild_id, connect_to).await;

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Error: songbird client missing")
        .clone();

    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        manager.remove(guild_id).await?;
        msg.reply(&ctx.http, "Left vc").await?;
    } else {
        msg.reply(&ctx.http, "Not in vc").await?;
    }

    Ok(())
}

#[command]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            msg.reply(&ctx.http, "Must provide url").await?;
            return Ok(());
        }
    };

    if !url.starts_with("http") {
        msg.reply(&ctx.http, "Must provide valid url").await?;
        return Ok(());
    }

    let manager = songbird::get(ctx)
        .await
        .expect("Error: songbird client missing")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let source = match songbird::ytdl(&url).await {
            Ok(source) => source,
            Err(why) => {
                println!("Error starting source: {:?}", why);
                msg.reply(&ctx.http, "Error sourcing ffmpeg").await?;

                return Ok(());
            }
        };

        handler.play_source(source);

        msg.reply(&ctx.http, "playing song").await?;
    } else {
        msg.reply(&ctx.http, "not in vc").await?;
    }

    Ok(())
}
