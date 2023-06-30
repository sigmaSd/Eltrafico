# TrafficToll

Frontend to eltrafico-tc that is similar to the original traffictoll

It uses a yaml config for the limits (with hot reload)

You can find an example of a config in *config.example.toml*

The cli usage is the same as the original traffictoll (except you need to specify eltrafico-tc path),
also this frontend should support the orignal yaml configs out of the box (WIP).


## Usage

```
TC=$path_to_eltrafico_tc deno run -A ./traffictoll.ts $netInterface config.example.toml
```
