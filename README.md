# Eltrafico
NetLimiter-like traffic shaping for Linux

This is a port of https://github.com/cryzed/TrafficToll to rust.

And a gui on top

<img src="https://github.com/sigmaSd/Eltrafico/raw/gui/gui.png" width="70%" height="70%">

# Usage
sudo eltrafico

# Howto
Choose the correct interface, and eltrafico will monitor it for active connections

Active program will automaticly show up

Choose your limits then confirm with set button

Default unit is "probably" bytes, so what you probably want is to specify the unit, exmp: "200kbps"

# Current State
Works on my pc (TM)
