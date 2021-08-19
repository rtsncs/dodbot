use crate::commands::{admin::*, music::*};
use serenity::framework::standard::macros::group;

#[group]
#[only_in(guilds)]
#[commands(
    play, join, leave, songinfo, queue, clear, stop, remove, mv, swap, skip, shuffle, seek, pause,
    resume, playlist, repeat, search
)]
pub struct Music;

#[group]
#[only_in(guilds)]
#[commands(setprefix)]
#[required_permissions(ADMINISTRATOR)]
pub struct Admin;
