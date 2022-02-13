mod commands;
mod config;
mod error;
mod events;
mod framework_functions;
mod guild;
mod music;
mod shared_data;

use framework_functions::*;
use songbird::SerenityInit;
use tracing::instrument;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

pub type Context<'a> = poise::Context<'a, shared_data::Data, error::Error>;

#[poise::command(prefix_command, hide_in_help, owners_only)]
async fn register(ctx: Context<'_>, #[flag] global: bool) -> Result<(), error::Error> {
    poise::builtins::register_application_commands(ctx, global).await?;
    Ok(())
}

#[tokio::main]
#[instrument]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_appender = tracing_appender::rolling::daily("./logs/", "dodbot_log");
    let (file_appender, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(fmt::layer().with_writer(std::io::stdout).compact())
        .with(
            fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .compact(),
        );

    tracing::subscriber::set_global_default(subscriber)?;

    let config: config::Config = toml::from_str(&std::fs::read_to_string("config.toml")?)?;
    let options = poise::FrameworkOptions {
        commands: vec![
            commands::general::help(),
            register(),
            commands::admin::roundrobin(),
            commands::admin::minecraftchannel(),
            commands::general::ping(),
            commands::general::minecraft(),
            commands::general::vps(),
            commands::music::join(),
            commands::music::leave(),
            commands::music::play(),
            commands::music::playlist(),
            commands::music::search(),
            commands::music::nowplaying(),
            commands::music::queue(),
            commands::music::myqueue(),
            commands::music::clear(),
            commands::music::stop(),
            commands::music::remove(),
            commands::music::mv(),
            commands::music::swap(),
            commands::music::skip(),
            commands::music::shuffle(),
            commands::music::seek(),
            commands::music::pause(),
            commands::music::resume(),
            commands::music::repeat(),
            commands::music::volume(),
            commands::music::lyrics(),
        ],
        pre_command: |ctx| Box::pin(async move { before(ctx) }),
        listener: |ctx, event, framework, data| {
            Box::pin(async move { events::event_listener(ctx, event, framework, data).await })
        },
        ..Default::default()
    };
    let framework = poise::Framework::build()
        .token(&config.token)
        .options(options)
        .user_data_setup(move |ctx, ready, framework| {
            Box::pin(async move { shared_data::Data::new(ctx, ready, framework, config).await })
        })
        .client_settings(|c| c.register_songbird())
        .build()
        .await?;

    let shard_manager = framework.shard_manager();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Error registering ctrl+c handler");
        tracing::info!("Shutting down!");
        shard_manager.lock().await.shutdown_all().await;
    });

    if let Err(why) = framework.start().await {
        tracing::error!("Error running client: {}", why);
    }

    Ok(())
}
