#![allow(clippy::wildcard_imports)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::blocks_in_if_conditions)]
#![allow(clippy::non_ascii_literal)]

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
            register(),
            commands::admin::roundrobin(),
            commands::admin::minecraftchannel(),
            commands::general::ping(),
            commands::general::minecraft(),
        ],
        // on_error: on_dispatch_error,
        pre_command: |ctx| Box::pin(async move { before(ctx) }),
        // post_command: after,
        // prefix_options: poise::PrefixFrameworkOptions {
        //     prefix: None,
        //     mention_as_prefix: true,
        //     dynamic_prefix: Some(dynamic_prefix),
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
