# Eltrafico
NetLimiter-like traffic shaping for Linux

This is a port of https://github.com/cryzed/TrafficToll to rust.

And a gui on top

<img src="./gui.png" width="70%" height="70%">

# Usage
sudo eltrafico

# Howto
Choose the correct interface, and eltrafico will monitor it for active connections

Active program will automaticly show up

Choose your limits then confirm with set button

Default unit is "probably" bytes, so what you probably want is to specify the unit, exp: "200kbps"

**Important**: Don't quit with `ctrl_c`, instead close the window so it can make the appropriate clean up (ToFix)

# Current State
Works on my pc (TM)
