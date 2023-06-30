import * as toml from "https://deno.land/std@0.192.0/toml/mod.ts";
import { ElTrafico, Unit } from "./eltrafico.ts";

interface Config {
  global: {
    interface: string;
    download: string;
    upload: string;
    download_minimum: string;
    upload_minimum: string;
    download_priority: number;
    upload_priority: number;
  };
  process: {
    rule_name: string;
    download: string;
    upload: string;
    download_minimum: string;
    upload_minimum: string;
    download_priority: number;
    upload_priority: number;
    match_exe: string;
  }[];
}

if (import.meta.main) {
  const configPath = Deno.args[0];
  if (!configPath) throw new Error("no config specified");

  const eltrafico = new ElTrafico();
  await limit({ configPath, eltrafico });

  const watcher = Deno.watchFs(configPath);
  let lastFutureJob = undefined;
  for await (const _event of watcher) {
    // update on any event but debounce a bit
    // only trigger reload after there are no events for 1 seconds
    clearInterval(lastFutureJob);
    lastFutureJob = setTimeout(
      async () => await limit({ configPath, eltrafico }),
      1000,
    );
  }
}

async function limit(
  { configPath, eltrafico }: { configPath: string; eltrafico: ElTrafico },
) {
  const config = toml.parse(
    Deno.readTextFileSync(configPath),
  ) as unknown as Config;
  //TODO: validate with zod

  await eltrafico.interface(config.global.interface);
  await eltrafico.limit({
    global: true,
    downloadLimit: parseValue(config.global.download),
    uploadLimit: parseValue(config.global.upload),
    downloadMinimum: parseValue(config.global.download_minimum),
    uploadMinimum: parseValue(config.global.upload_minimum),
  });
  for (const process of config.process) {
    await eltrafico.limit({
      name: process.match_exe,
      downloadLimit: parseValue(process.download),
      uploadLimit: parseValue(process.upload),
      downloadMinimum: parseValue(process.download_minimum),
      uploadMinimum: parseValue(process.upload_minimum),
    });
  }
}

export function parseValue(value: string) {
  const pattern = /(\d+)([A-Za-z]+)/;
  const result = value.match(pattern);

  if (result) {
    const value = parseInt(result[1]);
    const unit = result[2].toLowerCase() as Unit; //TODO validate
    return { value, unit };
  }

  throw new Error("Invalid value");
}
