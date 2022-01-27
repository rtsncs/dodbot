use async_minecraft_ping::ServerError as MinecraftError;
use lavalink_rs::error::LavalinkError;
use reqwest::Error as ReqwestError;
use serenity::prelude::SerenityError;
use sqlx::Error as SqlxError;

#[derive(Debug)]
pub enum Error {
    Serenity(SerenityError),
    Lavalink(LavalinkError),
    Reqwest(ReqwestError),
    Minecraft(MinecraftError),
    Sqlx(SqlxError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Serenity(err) => write!(f, "Serenity error: {err}"),
            Self::Lavalink(err) => write!(f, "Lavalink error: {err}"),
            Self::Reqwest(err) => write!(f, "Reqwest error: {err}"),
            Self::Minecraft(err) => write!(f, "Minecraft error: {err}"),
            Self::Sqlx(err) => write!(f, "Sqlx error: {err}"),
        }
    }
}

impl From<SerenityError> for Error {
    fn from(err: SerenityError) -> Self {
        Self::Serenity(err)
    }
}
impl From<LavalinkError> for Error {
    fn from(err: LavalinkError) -> Self {
        Self::Lavalink(err)
    }
}
impl From<ReqwestError> for Error {
    fn from(err: ReqwestError) -> Self {
        Self::Reqwest(err)
    }
}
impl From<MinecraftError> for Error {
    fn from(err: MinecraftError) -> Self {
        Self::Minecraft(err)
    }
}
impl From<SqlxError> for Error {
    fn from(err: SqlxError) -> Self {
        Self::Sqlx(err)
    }
}
