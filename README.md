# Eltrafico
NetLimiter-like traffic shaping for Linux

This is a port of https://github.com/cryzed/TrafficToll to rust.

And a gui on top

<img src="./gui.png" width="70%" height="70%">

# Usage
`sudo eltrafico`

# Howto
Choose the correct interface, and eltrafico will monitor it for active connections

Active program will automatically show up

Choose your limits then confirm with set button

Default unit is "probably" bytes, so what you probably want is to specify the unit, exp: "200kbps"

If [nethogs](https://github.com/raboof/nethogs) is installed on your system, `eltrafico` will use it automatically to show programs live network usage

## Current State
Works on my pc (TM)

## Dependencies
 - `iproute2`
 - `ifconfig`
 *optional*
 - [nethogs](https://github.com/raboof/nethogs)

## Releases
- Automatic releases by travis are uploaded https://github.com/sigmaSd/Eltrafico/releases

## Building
- needs gtk-dev: https://gtk-rs.org/docs/requirements.html
- cargo b --release

## [Changelog](./CHANGELOG.md)
