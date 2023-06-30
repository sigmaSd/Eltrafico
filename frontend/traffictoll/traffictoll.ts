import * as yaml from "https://deno.land/std@0.192.0/yaml/mod.ts";
import { ElTrafico, zProcess } from "./eltrafico.ts";
import { z } from "https://deno.land/x/zod@v3.21.4/mod.ts";

const zConfig = z.object({
  download: z.string().optional(),
  upload: z.string().optional(),
  "download-minimum": z.string().optional(),
  "upload-minimum": z.string().optional(),
  processes: z.record(z.string(), zProcess).optional(),
});
type Config = z.infer<typeof zConfig>;

if (import.meta.main) {
  const netInterface = Deno.args[0];
  if (!netInterface) throw new Error("no interface specified");

  const configPath = Deno.args[1];
  if (!configPath) throw new Error("no config specified");

  const eltrafico = new ElTrafico();
  try {
    await limit({ netInterface, configPath, eltrafico });
  } catch (e) {
    console.log("failed to apply limits:", e);
    Deno.exit(1);
  }

  const watcher = Deno.watchFs(configPath);
  let lastFutureJob = undefined;
  for await (const _event of watcher) {
    // update on any event but debounce a bit
    // only trigger reload after there are no events for 1 seconds
    clearInterval(lastFutureJob);
    lastFutureJob = setTimeout(
      async () => {
        try {
          await limit({ netInterface, configPath, eltrafico });
        } catch (e) {
          console.log("failed to apply limits:", e);
          // don't exit, it maybe a syntax error
        }
      },
      1000,
    );
  }
}

async function limit(
  { netInterface, configPath, eltrafico }: {
    netInterface: string;
    configPath: string;
    eltrafico: ElTrafico;
  },
) {
  const config: Config = zConfig.parse(yaml.parse(
    Deno.readTextFileSync(configPath),
  ));

  await eltrafico.interface(netInterface);
  await eltrafico.limitGlobal({
    download: config.download,
    upload: config.upload,
    "download-minimum": config["download-minimum"],
    "upload-minimum": config["upload-minimum"],
  });

  if (config.processes) {
    for (const [_name, process] of Object.entries(config.processes)) {
      await eltrafico.limit({
        match: process.match,
        download: process.download,
        upload: process.upload,
        "download-minimum": process["download-minimum"],
        "upload-minimum": process["upload-minimum"],
      });
    }
  }
}
