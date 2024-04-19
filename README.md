# Dødbot
Discord music bot wrriten in Rust, using Serenity library for the Discord API and Lavalink for playing music from YouTube.

![demo1](https://github.com/rtsncs/dodbot/assets/22866319/44cabfb3-ec44-4203-a3d4-fad9308386a7)
![demo2](https://github.com/rtsncs/dodbot/assets/22866319/1fa8f2d8-a3c1-4dc3-a62f-b7fb913a262d)

## Available commands
/join - joins the user's current voice channel

/leave - leaves the voice channel

/play [request] - plays the requested track, accepts YouTube links, plain text queries and Spotify links (it will search for the song on YouTube)

/playlist [request] - same as /play but for playlists

/search [query] - searches YouTube and shows the results

/nowplaying - shows information about currently playing track

/queue - lists the tracks in the queue

/myqueue - when round robin option is enabled, lists tracks enqueued by the users issuing the command

/clear - clears the queue

/stop - clears the queue and stops the currently playing track

/remove [n] - removes nth song from the queue

/move [n] [m] - moves nth track in the queue to mth position, or to the beginning of the queue if m is not specified

/swap [n] [m] - swaps nth and mth tracks in the queue

/skip - skips the currently playing track

/shuffle - shuffles the queue

/seek [time] - sets the position of the currently playing track to the given time in seconds

/pause - pause the currently playing track

/resume - resume the currently playing track

/loop [none | song | queue] - sets the loop mode, song means the currently playing track will be looped, queue - the whole queue, none disables looping

/volume [vol] - sets the volume to vol%, vol is a number between 0 and 1000

/lyrics [query] - searches Genius for lyrics

/roundrobin [true | false] - admin command, enables/disables round robin, if disabled tracks are played in the order they were enqueued, if enabled each user's tracks are played alternately
