# TrafficToll

Frontend to eltrafico-tc that is similar to the origianl traffictoll

It uses a toml config for the limits (with hot reload)

You can find an example of a config in *config.example.toml*

## Usage

```
TC=$path_to_eltrafico_tc deno run -A ./traffictoll.ts config.example.toml
```