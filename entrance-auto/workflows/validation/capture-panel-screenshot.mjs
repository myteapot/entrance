#!/usr/bin/env node
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import net from "node:net";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const scriptDir = dirname(__filename);
const rootDir = resolve(scriptDir, "../../..");
const srcDir = resolve(rootDir, "entrance-src");
const demoWorkflow = resolve(scriptDir, "run-local-mvp-demo.sh");

const defaults = {
  runId: process.env.ENTRANCE_PANEL_RUN_ID ?? utcRunId(),
  appRoot: process.env.ENTRANCE_PANEL_APP_ROOT ?? null,
  reportDir: process.env.ENTRANCE_PANEL_REPORT_DIR ?? resolve(rootDir, "entrance-auto/reports"),
  screenshotDir:
    process.env.ENTRANCE_PANEL_SCREENSHOT_DIR ?? resolve(rootDir, "entrance-auto/screenshots"),
  skipDemo: false,
  fullGates: false,
};

const args = parseArgs(process.argv.slice(2));
const runId = args.runId ?? defaults.runId;
const appRoot = resolve(args.appRoot ?? defaults.appRoot ?? resolve(rootDir, `entrance-auto/tmp/panel-screenshot-${runId}`));
const reportDir = resolve(args.reportDir ?? defaults.reportDir);
const screenshotDir = resolve(args.screenshotDir ?? defaults.screenshotDir);
const skipDemo = args.skipDemo ?? defaults.skipDemo;
const fullGates = args.fullGates ?? defaults.fullGates;

const screenshotPath = resolve(screenshotDir, `panel-local-mvp-${runId}.png`);
const metadataPath = resolve(reportDir, `panel-screenshot-${runId}.json`);
const summaryPath = resolve(reportDir, `panel-screenshot-${runId}.md`);
const runnerPath = resolve(appRoot, "panel-capture-electron.cjs");
const sourceCommit = run("git", ["-C", rootDir, "rev-parse", "--short", "HEAD"], {
  capture: true,
}).trim();

mkdirSync(appRoot, { recursive: true });
mkdirSync(reportDir, { recursive: true });
mkdirSync(screenshotDir, { recursive: true });

const children = [];

try {
  if (!skipDemo) {
    const demoArgs = [
      "--verify-golden",
      "--run-id",
      runId,
      "--app-root",
      appRoot,
      "--report-dir",
      reportDir,
    ];
    if (fullGates) {
      demoArgs.unshift("--full-gates");
    }
    run(demoWorkflow, demoArgs);
  } else if (!existsSync(resolve(appRoot, "data/entrance.db"))) {
    throw new Error(`--skip-demo requires an existing app root with data/entrance.db: ${appRoot}`);
  }

  const daemonPort = await reservePort();
  const vitePort = await reservePort();
  writeHttpPort(appRoot, daemonPort);

  const entranceBin = resolve(srcDir, "target/debug/entrance");
  if (!existsSync(entranceBin)) {
    run("cargo", ["build", "-q", "-p", "entrance-app", "--bin", "entrance"], { cwd: srcDir });
  }

  const daemon = spawnManaged(entranceBin, ["daemon", "http"], {
    cwd: srcDir,
    env: { ...process.env, ENTRANCE_APP_ROOT: appRoot },
    label: "daemon",
  });
  children.push(daemon);
  await waitForHttp(`http://127.0.0.1:${daemonPort}/health`, "daemon health");

  const panelUrl = `http://127.0.0.1:${vitePort}/`;
  const daemonUrl = `http://127.0.0.1:${daemonPort}`;
  const vite = spawnManaged(
    "pnpm",
    [
      "exec",
      "vite",
      "--config",
      "shell/gui/vite.config.ts",
      "--host",
      "127.0.0.1",
      "--port",
      String(vitePort),
      "--strictPort",
    ],
    {
      cwd: srcDir,
      env: { ...process.env, VITE_ENTRANCE_HTTP_URL: daemonUrl },
      label: "vite",
    },
  );
  children.push(vite);
  await waitForHttp(panelUrl, "Panel dev server");

  writeElectronRunner(runnerPath);
  const requireFromSrc = createRequire(resolve(srcDir, "package.json"));
  const electronBin = requireFromSrc("electron");
  run(electronBin, [runnerPath], {
    cwd: rootDir,
    env: {
      ...process.env,
      ENTRANCE_PANEL_URL: panelUrl,
      ENTRANCE_PANEL_SCREENSHOT: screenshotPath,
      ENTRANCE_PANEL_METADATA: metadataPath,
      ENTRANCE_PANEL_RUN_ID: runId,
      ENTRANCE_PANEL_APP_ROOT: appRoot,
      ENTRANCE_PANEL_DAEMON_URL: daemonUrl,
      ENTRANCE_PANEL_SOURCE_COMMIT: sourceCommit,
    },
  });

  const metadata = JSON.parse(readFileSync(metadataPath, "utf8"));
  const screenshot = statSync(screenshotPath);
  const report = {
    ...metadata,
    screenshot: {
      ...metadata.screenshot,
      path: screenshotPath,
      bytes: screenshot.size,
    },
    daemon: {
      url: daemonUrl,
      port: daemonPort,
    },
    vite: {
      url: panelUrl,
      port: vitePort,
    },
  };
  writeFileSync(metadataPath, `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(summaryPath, panelSummaryMarkdown(report));

  console.log("Panel screenshot captured.");
  console.log(`Screenshot: ${screenshotPath}`);
  console.log(`Metadata: ${metadataPath}`);
  console.log(`Summary: ${summaryPath}`);
} finally {
  for (const child of children.reverse()) {
    stopChild(child);
  }
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--run-id":
        parsed.runId = requireValue(argv, ++index, arg);
        break;
      case "--app-root":
        parsed.appRoot = requireValue(argv, ++index, arg);
        break;
      case "--report-dir":
        parsed.reportDir = requireValue(argv, ++index, arg);
        break;
      case "--screenshot-dir":
        parsed.screenshotDir = requireValue(argv, ++index, arg);
        break;
      case "--skip-demo":
        parsed.skipDemo = true;
        break;
      case "--full-gates":
        parsed.fullGates = true;
        break;
      case "-h":
      case "--help":
        printHelp();
        process.exit(0);
        break;
      default:
        throw new Error(`unknown argument: ${arg}`);
    }
  }
  return parsed;
}

function printHelp() {
  console.log(`Usage:
  entrance-auto/workflows/validation/capture-panel-screenshot.mjs [--full-gates] [--run-id <id>] [--app-root <path>] [--report-dir <path>] [--screenshot-dir <path>] [--skip-demo]

Captures the Panel Issue board from a clean local MVP app root. By default this
first runs run-local-mvp-demo.sh --verify-golden, then starts daemon HTTP,
Vite, and Electron capturePage. Screenshots stay under ignored
entrance-auto/screenshots by default. Use --full-gates to forward full source
validation into the local MVP demo workflow before capture.`);
}

function requireValue(argv, index, flag) {
  const value = argv[index];
  if (!value || value.startsWith("--")) {
    throw new Error(`missing value for ${flag}`);
  }
  return value;
}

function utcRunId() {
  return new Date().toISOString().replaceAll(/[-:]/g, "").replace(/\..+$/, "Z");
}

function run(command, args, options = {}) {
  const result = spawnSyncCompat(command, args, options);
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with exit code ${result.status}`);
  }
  return result.stdout ?? "";
}

function spawnSyncCompat(command, args, options) {
  const { spawnSync } = requireFromNodeChildProcess();
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? rootDir,
    env: options.env ?? process.env,
    encoding: "utf8",
    stdio: options.capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  return result;
}

function requireFromNodeChildProcess() {
  const require = createRequire(import.meta.url);
  return require("node:child_process");
}

function spawnManaged(command, args, options) {
  const child = spawn(command, args, {
    cwd: options.cwd,
    env: options.env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => process.stderr.write(`[${options.label}] ${chunk}`));
  child.stderr.on("data", (chunk) => process.stderr.write(`[${options.label}] ${chunk}`));
  child.on("exit", (code, signal) => {
    if (code !== null && code !== 0) {
      process.stderr.write(`[${options.label}] exited with code ${code}\n`);
    }
    if (signal) {
      process.stderr.write(`[${options.label}] exited by signal ${signal}\n`);
    }
  });
  return child;
}

function stopChild(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  child.kill();
}

async function reservePort() {
  return new Promise((resolvePort, reject) => {
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : null;
      server.close(() => {
        if (!port) {
          reject(new Error("failed to reserve local port"));
          return;
        }
        resolvePort(port);
      });
    });
    server.on("error", reject);
  });
}

function writeHttpPort(root, port) {
  const configPath = resolve(root, "entrance.toml");
  const config = existsSync(configPath) ? readFileSync(configPath, "utf8") : "";
  let next = config;
  if (/http_port\s*=\s*\d+/.test(next)) {
    next = next.replace(/http_port\s*=\s*\d+/, `http_port = ${port}`);
  } else if (/^\[hive\]\s*$/m.test(next)) {
    next = next.replace(/^\[hive\]\s*$/m, `[hive]\nhttp_port = ${port}`);
  } else {
    next = `${next.trimEnd()}\n\n[hive]\nhttp_port = ${port}\n`;
  }
  writeFileSync(configPath, next);
}

async function waitForHttp(url, label) {
  const started = Date.now();
  let lastError = null;
  while (Date.now() - started < 30_000) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }
      lastError = new Error(`${label} returned HTTP ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 250));
  }
  throw new Error(`timed out waiting for ${label}: ${lastError?.message ?? "unknown error"}`);
}

function writeElectronRunner(path) {
  writeFileSync(path, `const { app, BrowserWindow } = require("electron");
const { writeFile, stat } = require("node:fs/promises");

const panelUrl = process.env.ENTRANCE_PANEL_URL;
const screenshotPath = process.env.ENTRANCE_PANEL_SCREENSHOT;
const metadataPath = process.env.ENTRANCE_PANEL_METADATA;
const runId = process.env.ENTRANCE_PANEL_RUN_ID;
const appRoot = process.env.ENTRANCE_PANEL_APP_ROOT;
const daemonUrl = process.env.ENTRANCE_PANEL_DAEMON_URL;
const sourceCommit = process.env.ENTRANCE_PANEL_SOURCE_COMMIT;
const consoleMessages = [];
let win;

app.commandLine.appendSwitch("disable-gpu");

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForCondition(win, expression, label) {
  const started = Date.now();
  while (Date.now() - started < 20_000) {
    const ok = await win.webContents.executeJavaScript(expression);
    if (ok) {
      return;
    }
    await sleep(250);
  }
  throw new Error("timed out waiting for " + label);
}

async function main() {
  await app.whenReady();
  win = new BrowserWindow({
    show: false,
    width: 1440,
    height: 1000,
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  win.webContents.on("console-message", (_event, level, message) => {
    consoleMessages.push({ level, message });
  });

  await win.loadURL(panelUrl);
  await waitForCondition(
    win,
    "Boolean(document.querySelector('button')) && document.body.textContent.includes('Entrance V2')",
    "initial shell",
  );

  await win.webContents.executeJavaScript(\`
  (() => {
    const button = Array.from(document.querySelectorAll('button'))
      .find((item) => (item.textContent || '').includes('Panel'));
    if (!button) return false;
    button.click();
    return true;
  })()
  \`);

  await waitForCondition(
    win,
    "Boolean(document.querySelector('[data-testid=panel-run-fixture-demo]')) && Boolean(document.querySelector('[data-testid^=loop-control-detail-]')) && Boolean(document.querySelector('[data-testid^=loop-dashboard-detail-]')) && Boolean(document.querySelector('[data-testid^=evidence-drilldown-detail-]')) && Boolean(document.querySelector('[data-testid^=evidence-manifest-detail-]')) && Boolean(document.querySelector('[data-testid^=issue-timeline-detail-]')) && Boolean(document.querySelector('[data-testid^=runtime-preflight-detail-]')) && Boolean(document.querySelector('[data-testid^=worker-lifecycle-detail-]')) && document.body.textContent.includes('Entrance remote fixture demo')",
    "Panel issue board",
  );
  await win.webContents.executeJavaScript(\`
  (() => {
    const card = Array.from(document.querySelectorAll('.issue-card'))
      .find((item) => (item.textContent || '').includes('Entrance MVP demo'));
    const button = card ? card.querySelector('[data-testid^="issue-action-board-details-"]') : null;
    if (!button) return false;
    button.click();
    return true;
  })()
  \`);
  await waitForCondition(
    win,
    "(() => { const detail = document.querySelector('.panel--detail'); const text = detail ? (detail.textContent || '') : ''; return Boolean(detail) && text.includes('Entrance MVP demo') && text.includes('Reviewer kept the candidate') && text.includes('loop_control.v1') && text.includes('packets 4 / admissions 4 / evidence 3 / verdicts 1') && text.includes('evidence_drilldown.v1') && text.includes('evidence_manifest.v1') && text.includes('issue_timeline.v1') && Boolean(detail.querySelector('[data-testid^=loop-control-detail-]')) && Boolean(detail.querySelector('[data-testid^=loop-dashboard-detail-]')) && Boolean(detail.querySelector('[data-testid^=evidence-drilldown-detail-]')) && Boolean(detail.querySelector('[data-testid^=evidence-manifest-detail-]')) && Boolean(detail.querySelector('[data-testid^=issue-timeline-detail-]')) && Boolean(detail.querySelector('[data-testid^=runtime-preflight-detail-]')) && Boolean(detail.querySelector('[data-testid^=worker-lifecycle-detail-]')); })()",
    "local MVP issue detail",
  );
  await win.webContents.executeJavaScript(\`
  (() => {
    const detail = document.querySelector('.panel--detail');
    const timeline = detail ? detail.querySelector('[data-testid^="issue-timeline-detail-"]') : null;
    if (!timeline) return false;
    timeline.scrollIntoView({ block: 'center', inline: 'nearest' });
    return true;
  })()
  \`);
  await sleep(500);

  const page = await win.webContents.executeJavaScript(\`
(() => {
  const text = (document.body.textContent || '').replace(/\\\\s+/g, ' ').trim();
  const detail = document.querySelector('.panel--detail');
  const detailText = detail ? (detail.textContent || '').replace(/\\\\s+/g, ' ').trim() : '';
  const detailQuery = (selector) => detail ? detail.querySelector(selector) : null;
  const byTestId = (id) => {
    const element = document.querySelector('[data-testid="' + id + '"]');
    return element ? {
      text: (element.textContent || '').replace(/\\\\s+/g, ' ').trim(),
      title: element.getAttribute('title'),
      disabled: element.hasAttribute('disabled'),
    } : null;
  };
  const assertions = {
    panel_run_fixture_visible: Boolean(document.querySelector('[data-testid="panel-run-fixture-demo"]')),
    local_mvp_issue_visible: text.includes('Entrance MVP demo'),
    local_workbench_summary_visible: Boolean(document.querySelector('[data-testid="local-workbench-summary"]')),
    selected_local_mvp_detail_visible: detailText.includes('Entrance MVP demo'),
    issue_transition_policy_visible: Boolean(detailQuery('[data-testid^="issue-transition-policy-detail-"]')),
    issue_transition_policy_actions_visible: Boolean(detailQuery('[data-testid^="issue-transition-policy-action-"]')),
    issue_transition_policy_allowed_visible: detailText.includes('allowed'),
    issue_transition_policy_budget_visible: detailText.includes('reviewer 0/3'),
    reviewer_control_visible: Boolean(detailQuery('[data-testid^="loop-control-detail-"]')),
    reviewer_control_schema_visible: detailText.includes('loop_control.v1'),
    reviewer_control_budget_visible: detailText.includes('reviewer 0/3'),
    reviewer_control_runtime_gate_visible: detailText.includes('runtime_policy_ready ok'),
    reviewer_control_lifecycle_gate_visible: detailText.includes('lifecycle succeeded'),
    reviewer_control_evidence_gate_visible: detailText.includes('evidence ok'),
    reviewer_control_score_visible: detailText.includes('stage 1.00') && detailText.includes('runtime 1.00') && detailText.includes('evidence 1.00') && detailText.includes('admission 1.00'),
    reviewer_control_options_visible: Boolean(detailQuery('[data-testid^="loop-control-option-"][data-testid$="-A"]')) && Boolean(detailQuery('[data-testid^="loop-control-option-"][data-testid$="-B"]')) && Boolean(detailQuery('[data-testid^="loop-control-option-"][data-testid$="-C"]')),
    reviewer_control_option_a_visible: detailText.includes('A. retry with changed boundary'),
    reviewer_control_option_b_visible: detailText.includes('B. request human review'),
    reviewer_control_option_c_visible: detailText.includes('C. keep blocked'),
    loop_dashboard_visible: Boolean(detailQuery('[data-testid^="loop-dashboard-detail-"]')),
    loop_dashboard_developer_visible: Boolean(detailQuery('[data-testid^="loop-dashboard-agent-"][data-testid$="-developer"]')),
    loop_dashboard_reviewer_visible: Boolean(detailQuery('[data-testid^="loop-dashboard-agent-"][data-testid$="-reviewer"]')),
    loop_dashboard_budget_visible: detailText.includes('review budget 0/3'),
    loop_dashboard_round_visible: Boolean(detailQuery('[data-testid^="loop-dashboard-round-"]')),
    loop_dashboard_round_groups_visible: detailText.includes('packets 4 / admissions 4 / evidence 3 / verdicts 1') && detailText.includes('packet kernel kernel->explorer PREFLIGHT_PACKET admitted') && detailText.includes('verdict keep all_gates_passed'),
    evidence_drilldown_visible: Boolean(detailQuery('[data-testid^="evidence-drilldown-detail-"]')),
    evidence_drilldown_item_visible: Boolean(detailQuery('[data-testid^="evidence-drilldown-item-"]')),
    evidence_drilldown_receipt_visible: detailText.includes('receipt developer implement-admitted-candidate gates'),
    evidence_drilldown_payload_visible: detailText.includes('payload +') && detailText.includes('worker'),
    evidence_manifest_visible: Boolean(detailQuery('[data-testid^="evidence-manifest-detail-"]')),
    evidence_manifest_entry_visible: Boolean(detailQuery('[data-testid^="evidence-manifest-entry-"]')),
    evidence_manifest_payload_visible: detailText.includes('evidence.payload') && detailText.includes('payload'),
    evidence_manifest_receipt_visible: detailText.includes('worker.receipt') && detailText.includes('receipt'),
    evidence_manifest_digest_visible: detailText.includes('sha256'),
    issue_timeline_visible: Boolean(detailQuery('[data-testid^="issue-timeline-detail-"]')),
    issue_timeline_item_visible: Boolean(detailQuery('[data-testid^="issue-timeline-item-"]')),
    issue_timeline_comment_visible: detailText.includes('stage_comment') && detailText.includes('comment #'),
    issue_timeline_evidence_visible: detailText.includes('execution_packet') && detailText.includes('evidence #'),
    issue_timeline_verdict_visible: detailText.includes('Verdict #') && detailText.includes('decision keep'),
    issue_timeline_round_visible: Boolean(detailQuery('[data-testid^="issue-timeline-round-"]')),
    issue_timeline_round_group_visible: detailText.includes('round 1') && detailText.includes('comments') && detailText.includes('verdicts 1'),
    issue_timeline_decision_chip_visible: detailText.includes('decision comment'),
    issue_timeline_receipt_chip_visible: detailText.includes('receipts 0'),
    issue_timeline_permalink_visible: Boolean(detailQuery('[data-testid^="issue-timeline-item-permalink-"]')) && detailText.includes('permalink'),
    issue_timeline_in_view: (() => {
      const timeline = detailQuery('[data-testid^="issue-timeline-detail-"]');
      if (!timeline) return false;
      const rect = timeline.getBoundingClientRect();
      return rect.bottom > 0 && rect.top < innerHeight;
    })(),
    runtime_preflight_visible: Boolean(detailQuery('[data-testid^="runtime-preflight-detail-"]')),
    runtime_preflight_gate_visible: detailText.includes('runtime_policy_ready'),
    runtime_preflight_route_visible: detailText.includes('kernel -> explorer'),
    runtime_capability_visible: Boolean(detailQuery('[data-testid^="runtime-capability-preview-"]')),
    runtime_capability_schema_visible: detailText.includes('runtime_capability_preview.v1'),
    runtime_capability_worker_visible: detailText.includes('worker spawn ready'),
    runtime_capability_human_visible: detailText.includes('human human_confirmed'),
    worker_lifecycle_visible: Boolean(detailQuery('[data-testid^="worker-lifecycle-detail-"]')),
    worker_lifecycle_explorer_visible: Boolean(detailQuery('[data-testid^="worker-lifecycle-role-"][data-testid$="-explorer"]')),
    worker_lifecycle_developer_visible: Boolean(detailQuery('[data-testid^="worker-lifecycle-role-"][data-testid$="-developer"]')),
    worker_lifecycle_reviewer_visible: Boolean(detailQuery('[data-testid^="worker-lifecycle-role-"][data-testid$="-reviewer"]')),
    worker_lifecycle_budget_visible: detailText.includes('review budget 0/3'),
    todo_column_visible: text.includes('Todo'),
    done_column_visible: text.includes('Done'),
    reviewer_keep_visible: detailText.includes('Reviewer kept the candidate'),
  };
  return {
    title: document.title,
    url: location.href,
    viewport: { width: innerWidth, height: innerHeight },
    assertions,
    controls: {
      panel_run_fixture: byTestId('panel-run-fixture-demo'),
      issue_empty_run_fixture: byTestId('issue-empty-run-fixture-demo'),
    },
    excerpts: {
      main: (document.querySelector('main')?.textContent || '').replace(/\\\\s+/g, ' ').trim().slice(0, 1600),
      detail: detailText.slice(0, 1600),
    },
  };
})()
\`);

  const failedAssertions = Object.entries(page.assertions)
    .filter(([, value]) => !value)
    .map(([name]) => name);

  const image = await win.webContents.capturePage();
  await writeFile(screenshotPath, image.toPNG());
  const screenshot = await stat(screenshotPath);
  const metadata = {
    schema_version: "entrance.auto.panel_screenshot.v1",
    run_id: runId,
    source_commit: sourceCommit,
    captured_at: new Date().toISOString(),
    app_root: appRoot,
    panel_url: panelUrl,
    daemon_url: daemonUrl,
    screenshot: {
      path: screenshotPath,
      bytes: screenshot.size,
      mime_type: "image/png",
    },
    page,
    failed_assertions: failedAssertions,
    console_errors: consoleMessages.filter((entry) => entry.level >= 2),
  };
  await writeFile(metadataPath, JSON.stringify(metadata, null, 2) + "\\n");

  if (failedAssertions.length) {
    throw new Error("Panel screenshot assertions failed: " + failedAssertions.join(", "));
  }

  app.exit(0);
}

main().catch(async (error) => {
  let debug = null;
  if (win && !win.isDestroyed()) {
    debug = await win.webContents.executeJavaScript(\`
      (() => ({
        title: document.title,
        url: location.href,
        text: (document.body.textContent || '').replace(/\\\\s+/g, ' ').trim().slice(0, 2000),
        buttons: Array.from(document.querySelectorAll('button')).map((button) => ({
          text: (button.textContent || '').replace(/\\\\s+/g, ' ').trim(),
          testid: button.getAttribute('data-testid'),
          title: button.getAttribute('title'),
        })).slice(0, 40),
        testids: Array.from(document.querySelectorAll('[data-testid]')).map((element) => element.getAttribute('data-testid')).slice(0, 80),
      }))()
    \`).catch((debugError) => ({ error: String(debugError) }));
  }
  await writeFile(metadataPath, JSON.stringify({
    schema_version: "entrance.auto.panel_screenshot.v1",
    run_id: runId,
    source_commit: sourceCommit,
    captured_at: new Date().toISOString(),
    app_root: appRoot,
    panel_url: panelUrl,
    daemon_url: daemonUrl,
    failed_assertions: ["capture_failed"],
    error: String(error?.stack || error),
    debug,
    console_errors: consoleMessages.filter((entry) => entry.level >= 2),
  }, null, 2) + "\\n").catch(() => {});
  console.error(error?.stack || error);
  app.exit(1);
});
`);
}

function panelSummaryMarkdown(report) {
  return [
    "# Entrance Panel Screenshot Report",
    "",
    `- Run id: ${report.run_id}`,
    `- Source commit: ${report.source_commit}`,
    `- App root: \`${report.app_root}\``,
    `- Panel URL: ${report.panel_url}`,
    `- Screenshot: \`${report.screenshot.path}\``,
    `- Screenshot bytes: ${report.screenshot.bytes}`,
    `- Failed assertions: ${report.failed_assertions.length ? report.failed_assertions.join(", ") : "none"}`,
    "",
    "## Assertions",
    "",
    ...Object.entries(report.page.assertions).map(([key, value]) => `- ${key}: ${value}`),
    "",
  ].join("\n");
}
