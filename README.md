# Eltrafico
NetLimiter-like traffic shaping for Linux

This is a port of https://github.com/cryzed/TrafficToll to rust.

It should be now about 95% ported 

Currently Missing: 
 - Logging
 - Parsing Yaml 

 ## Usage
 `sudo eltrafico device config Optional<delay>`
 
 `sudo -E cargo device config`
 
  Exmaple of usage:
  
    # scan for active connections each second
    sudo eltrafico wlp3s0 config 1
 
    # scan continuously for active connections
    sudo eltrafico wlp3s0 config
 
 You need  a config file here is an example:

    # set global limit
    global d: 400kbps u: 200kbps

    # apps (use the command name used to invoke the program)
    fiefox d: 300kbps
    utorrent u: 200kbps

## GUI branch [WIP]
<img src="https://github.com/sigmaSd/Eltrafico/raw/gui/gui.png" width="70%" height="70%">
