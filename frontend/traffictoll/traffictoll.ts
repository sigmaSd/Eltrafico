import * as yaml from "https://deno.land/std@0.192.0/yaml/mod.ts";
import { ElTrafico, Process } from "./eltrafico.ts";

//TODO: validate with zod

interface Config {
  download: string;
  upload: string;
  "download-minimum": string;
  "upload-minimum": string;
  processes: Record<string, Process>;
}

if (import.meta.main) {
  const netInterface = Deno.args[0];
  if (!netInterface) throw new Error("no interface specified");

  const configPath = Deno.args[1];
  if (!configPath) throw new Error("no config specified");

  const eltrafico = new ElTrafico();
  await limit({ netInterface, configPath, eltrafico });

  const watcher = Deno.watchFs(configPath);
  let lastFutureJob = undefined;
  for await (const _event of watcher) {
    // update on any event but debounce a bit
    // only trigger reload after there are no events for 1 seconds
    clearInterval(lastFutureJob);
    lastFutureJob = setTimeout(
      async () => await limit({ netInterface, configPath, eltrafico }),
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
  const config = yaml.parse(
    Deno.readTextFileSync(configPath),
  ) as unknown as Config;

  await eltrafico.interface(netInterface);
  await eltrafico.limitGlobal({
    download: config.download,
    upload: config.upload,
    "download-minimum": config["download-minimum"],
    "upload-minimum": config["upload-minimum"],
  });

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
