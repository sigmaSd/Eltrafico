# Eltrafico
NetLimiter-like traffic shaping for Linux

This is a port of https://github.com/cryzed/TrafficToll to rust, its a WIP

 # Usage
 `eltrafico config`
 
 you need  a config file here is an example:

    # set global limit
    global d: 400kbps u: 200kbps

    # apps
    fiefox d: 300kbps
    utorrent u: 200kbps
