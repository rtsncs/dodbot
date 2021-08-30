use crate::commands::{admin::*, general::*, music::*};
use serenity::framework::standard::macros::group;

#[group]
#[only_in(guilds)]
#[commands(
    play, join, leave, songinfo, queue, clear, stop, remove, mv, swap, skip, shuffle, seek, pause,
    resume, playlist, repeat, search, volume, myqueue
)]
pub struct Music;

#[group]
#[only_in(guilds)]
#[commands(setprefix, roundrobin)]
#[required_permissions(ADMINISTRATOR)]
pub struct Admin;

#[group]
#[only_in(guilds)]
#[commands(ping)]
pub struct General;
