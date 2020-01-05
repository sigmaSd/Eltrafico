# Eltrafico
NetLimiter-like traffic shaping for Linux

This is a port of https://github.com/cryzed/TrafficToll to rust.

It should be now about 95% ported (missing yaml parsing which I'm not fun of, I probably will just skip to gui).

 # Usage
 `eltrafico device config Optional<delay>`
 
  Exmaple of usage:
  
    # scan for active connections each second
    eltrafico wlp3s0 config 1
 
    # scan continuously for active connections
    eltrafico wlp3s0 config
 
 You need  a config file here is an example:

    # set global limit
    global d: 400kbps u: 200kbps

    # apps (use the command name used to invoke the program)
    fiefox d: 300kbps
    utorrent u: 200kbps
