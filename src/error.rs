use async_minecraft_ping::ServerError as MinecraftError;
use lavalink_rs::error::LavalinkError;
use reqwest::Error as ReqwestError;
use rspotify::model::idtypes::IdError as SpotifyIdError;
use rspotify::ClientError as SpotifyClientError;
use serenity::prelude::SerenityError;
use songbird::error::JoinError as SongbirdError;
use sqlx::Error as SqlxError;

#[derive(Debug)]
pub enum Error {
    Serenity(SerenityError),
    Lavalink(LavalinkError),
    Reqwest(ReqwestError),
    Minecraft(MinecraftError),
    Sqlx(SqlxError),
    Songbird(SongbirdError),
    Spotify(String),

    JoinError(String),
    CommandError(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Serenity(err) => write!(f, "Serenity error: {err}"),
            Self::Lavalink(err) => write!(f, "Lavalink error: {err}"),
            Self::Reqwest(err) => write!(f, "Reqwest error: {err}"),
            Self::Minecraft(err) => write!(f, "Minecraft error: {err}"),
            Self::Sqlx(err) => write!(f, "Sqlx error: {err}"),
            Self::Songbird(err) => write!(f, "Songbird error: {err}"),
            Self::Spotify(err) => write!(f, "Spotify error: {err}"),

            Self::JoinError(err) => write!(f, "Error joining voice channel: {err}"),
            Self::CommandError(err) => write!(f, "Error: {err}"),
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
impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Self::CommandError(err.to_string())
    }
}
impl From<SongbirdError> for Error {
    fn from(err: SongbirdError) -> Self {
        Self::Songbird(err)
    }
}
impl From<SpotifyClientError> for Error {
    fn from(err: SpotifyClientError) -> Self {
        Self::Spotify(err.to_string())
    }
}
impl From<SpotifyIdError> for Error {
    fn from(err: SpotifyIdError) -> Self {
        Self::Spotify(err.to_string())
    }
}
