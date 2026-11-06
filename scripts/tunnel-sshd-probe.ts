#!/usr/bin/env -S deno run -A
import $, { KillController } from "jsr:@david/dax@0.50.2";

/** Quote paths for the shell OpenSSH itself uses for ProxyCommand. Dax has
 * already escaped the outer argv; this second boundary belongs to ssh. */
function shellQuote(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

/** OpenSSH config uses double-quoted words, independently of shell quoting. */
function configQuote(value: string): string {
  if (/[\r\n\0]/.test(value)) throw new Error("sshd config word contains a line break or NUL");
  return `"${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
}

/** Require a real server at an absolute path; a skipped probe proves nothing. */
async function serverPath(): Promise<string> {
  // Apple's server requires an audit session even for an unprivileged inetd
  // connection over pipes. Portable OpenSSH keeps this fixture unprivileged;
  // the client below remains the one macOS ships.
  const paths = Deno.build.os === "darwin"
    ? ["/opt/homebrew/opt/openssh/sbin/sshd", "/usr/local/opt/openssh/sbin/sshd"]
    : ["/usr/sbin/sshd", "/usr/local/sbin/sshd"];
  for (const path of paths) {
    try {
      if ((await Deno.stat(path)).isFile) return path;
    } catch (error) {
      if (!(error instanceof Deno.errors.NotFound)) throw error;
    }
  }
  throw new Error("real sshd is required for the unprivileged inetd probe");
}

/** One real authenticated session round-trips stdin using only owned keys.
 * ssh starts its inetd-mode server as its own proxy child: no daemon or
 * network listener is created, and normal EOF closes both processes. */
async function main(scratch: string) {
  if (Deno.uid() === 0) throw new Error("the inetd probe must run without root privileges");
  const sshd = await serverPath();
  const kill = new KillController();
  const interrupted = () => kill.kill();
  for (const signal of ["SIGINT", "SIGTERM"] as const) Deno.addSignalListener(signal, interrupted);
  const root = await Deno.makeTempDir({ dir: scratch, prefix: "sshd-probe-" });
  try {
    const clientKey = `${root}/client-key`;
    const hostKey = `${root}/host-key`;
    const authorized = `${root}/authorized_keys`;
    const config = `${root}/sshd_config`;
    await $`ssh-keygen -q -t ed25519 -N '' -f ${clientKey}`.signal(kill.signal).timeout("10s");
    await $`ssh-keygen -q -t ed25519 -N '' -f ${hostKey}`.signal(kill.signal).timeout("10s");
    await Deno.copyFile(`${clientKey}.pub`, authorized);
    await Deno.chmod(authorized, 0o600);
    await Deno.writeTextFile(
      config,
      [
        `HostKey ${configQuote(hostKey)}`,
        `AuthorizedKeysFile ${configQuote(authorized)}`,
        "UsePAM no",
        "StrictModes no",
        "PidFile none",
        "PubkeyAuthentication yes",
        "PasswordAuthentication no",
        "KbdInteractiveAuthentication no",
        "PermitUserEnvironment no",
        "PrintMotd no",
        "LogLevel ERROR",
        "",
      ].join("\n"),
      { mode: 0o600 },
    );
    const user = (await $`id -un`.signal(kill.signal).timeout("5s").text()).trim();
    const proxy = `ProxyCommand=${shellQuote(sshd)} -i -e -f ${shellQuote(config)}`;
    const marker = `farhelm-sshd-inetd-${crypto.randomUUID()}\n`;
    const started = performance.now();
    const client = Deno.build.os === "darwin" ? "/usr/bin/ssh" : "ssh";
    const result = await $`${client} -F /dev/null -T -i ${clientKey} \
      -o ${proxy} -o IdentitiesOnly=yes -o IdentityAgent=none \
      -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
      -o GlobalKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=10 \
      -o ControlMaster=no -o ControlPath=none -l ${user} probe.invalid cat`
      .stdinText(marker)
      .stdout("piped")
      .stderr("piped")
      .signal(kill.signal)
      .timeout("20s")
      .noThrow();
    if (result.code !== 0 || result.stdout !== marker) {
      throw new Error(
        `inetd round trip failed: code=${result.code}, stdout=${JSON.stringify(result.stdout)}, stderr=${
          JSON.stringify(result.stderr.slice(-8192))
        }`,
      );
    }
    console.log(JSON.stringify({
      proof: "real unprivileged sshd -i authenticated stdin echo",
      os: Deno.build.os,
      sshd,
      bytes: new TextEncoder().encode(marker).length,
      elapsed_ms: Math.round(performance.now() - started),
      exit_code: result.code,
      listener_created: false,
    }));
  } finally {
    kill.kill();
    // Remove only this invocation's random directory, including its private
    // keys. Failures retain their bounded diagnostics in the recorder output.
    await Deno.remove(root, { recursive: true });
    for (const signal of ["SIGINT", "SIGTERM"] as const) Deno.removeSignalListener(signal, interrupted);
  }
}

Deno.test("proxy command and config paths retain quotes and spaces", () => {
  if (shellQuote("a'b c") !== "'a'\\''b c'") throw new Error("shell path quoting changed");
  if (configQuote('a"b\\c') !== '"a\\"b\\\\c"') throw new Error("config path quoting changed");
  for (const value of ["a\nb", "a\rb", "a\0b"]) {
    let rejected = false;
    try {
      configQuote(value);
    } catch {
      rejected = true;
    }
    if (!rejected) throw new Error("config line injection accepted");
  }
});

if (import.meta.main) {
  const scratch = Deno.args[0] ?? new URL(".", import.meta.url).pathname;
  await main(scratch);
}
