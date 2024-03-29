import { z } from "https://deno.land/x/zod@v3.21.4/mod.ts";

export const zProcess = z.object({
  // the only required prop, can be made optional as well
  match: z.object({ name: z.string().optional() }).array(),
  download: z.string().optional(),
  upload: z.string().optional(),
  "download-minimum": z.string().optional(),
  "upload-minimum": z.string().optional(),
  //TODO: use these
  "download-priority": z.number().optional(),
  "upload-priority": z.number().optional(),
});
export type Process = z.infer<typeof zProcess>;

function findEltraficoTc() {
  return Deno.env.get("TC") || "eltrafico-tc";
}

//TODO: add reset

export class ElTrafico {
  #tc: Deno.ChildProcess;
  #reader;
  #writer;
  constructor() {
    this.#tc = new Deno.Command("pkexec", {
      args: [findEltraficoTc()],
      stdout: "piped",
      stdin: "piped",
      stderr: "inherit",
    }).spawn();
    this.#reader = this.#tc.stdout.getReader();
    this.#writer = this.#tc.stdin.getWriter();
  }
  async limitGlobal(global: Omit<Process, "match">) {
    const startMsg = "Global: ";
    const limitAction = `${startMsg} ${utn(global.download)} ${
      utn(global.upload)
    } ${utn(global["download-minimum"])} ${utn(global["upload-minimum"])}`;
    await this.#write(limitAction);
  }
  async limit(process: Process) {
    //TODO: use all match names
    const startMsg = `Program: ${utn(process.match[0].name)}`;
    const limitAction = `${startMsg} ${utn(process.download)} ${
      utn(process.upload)
    } ${utn(process["download-minimum"])} ${utn(process["upload-minimum"])}`;

    await this.#write(limitAction);
  }
  async stop() {
    await this.#write("Stop");
  }
  async interface(name: string) {
    await this.#write(`Interface: ${name}`);
  }
  async poll() {
    const data = await this.#read();

    if (data == "Stop") {
      return { stop: true };
    }

    return (data.split("\n").filter((l) => l).map((line) => {
      return { name: line.split("ProgramEntry: ")[1] };
    }));
  }
  async #read() {
    return await this.#reader.read().then((data) =>
      // the data is small
      // it should be done in one read
      // NOTE: this assumption might not hold
      new TextDecoder().decode(data.value)
    );
  }
  async #write(data: string) {
    return await this.#writer.write(
      new TextEncoder().encode(data + "\n"),
    );
  }
}

/** Undefined to None */
function utn(
  maybeValue: string | undefined,
) {
  if (maybeValue === undefined) return "None";
  return maybeValue;
}
