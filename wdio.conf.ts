import type { Options } from "@wdio/types";
import { spawn, type ChildProcess } from "child_process";

let tauriDriver: ChildProcess;

export const config: Options.Testrunner = {
  specs: ["./e2e/**/*.test.ts"],
  maxInstances: 1,
  hostname: "localhost",
  port: 4444,
  capabilities: [
    {
      // @ts-expect-error custom tauri capability
      "tauri:options": {
        application: "./src-tauri/target/debug/folder-search",
      },
    },
  ],
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: 60000,
  },
  reporters: ["spec"],

  onPrepare: () =>
    new Promise<void>((resolve) => {
      tauriDriver = spawn("tauri-driver", [], {
        stdio: [null, process.stdout, process.stderr],
      });
      setTimeout(resolve, 2000);
    }),

  onComplete: () => {
    tauriDriver.kill();
  },
};
