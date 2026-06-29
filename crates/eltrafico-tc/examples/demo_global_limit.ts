const targetDir = Deno.env.get("CARGO_TARGET_DIR") || "target";
const binaryPath = `${targetDir}/debug/eltrafico-tc`;

const cmd = new Deno.Command("pkexec", {
  args: [binaryPath],
  stdin: "piped",
  stdout: "piped",
});

const child = cmd.spawn();
const writer = child.stdin.getWriter();
const encoder = new TextEncoder();
const writeLine = (line: string) => writer.write(encoder.encode(line + "\n"));

await writeLine("Interface: wlan0");
await writeLine("Global: 500kbps None None None None None");
console.log("Set global download to 500kbps");

await new Promise((r) => setTimeout(r, 3000));

await writeLine("Global: None None None None None None");
console.log("Cleared global download limit");

await new Promise((r) => setTimeout(r, 3000));

await writeLine("Stop");
console.log("Sent Stop signal");

writer.close();

const { stdout } = await child.output();
console.log("--- stdout ---");
console.log(new TextDecoder().decode(stdout));
