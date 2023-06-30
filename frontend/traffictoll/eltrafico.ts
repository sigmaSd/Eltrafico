export type Unit = "bps" | "kbps" | "mbps";
export type Value = { value: number; unit: Unit };
export interface Program {
  name?: string;
  global?: boolean;
  downloadLimit?: Value;
  uploadLimit?: Value;
  downloadMinimum?: Value;
  uploadMinimum?: Value;
}

function findEltraficoTc() {
  return Deno.env.get("TC") || "eltrafico-tc";
}

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
  async limit(program: Program) {
    const startMsg = program.global ? "Global: " : `Program: ${program.name}`;
    const limitAction = `${startMsg} ${getLimit(program.downloadLimit)} ${
      getLimit(program.uploadLimit)
    } ${getLimit(program.downloadMinimum)} ${getLimit(program.uploadMinimum)}`;

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

function getLimit(limitAndUnit?: { value: number; unit: Unit }) {
  if (limitAndUnit) {
    return limitAndUnit.value.toString() + limitAndUnit.unit;
  } else {
    return "None";
  }
}
