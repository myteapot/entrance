#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const scriptDir = dirname(__filename);
const rootDir = resolve(scriptDir, "../../..");
const srcDir = resolve(rootDir, "entrance-src");
const args = parseArgs(process.argv.slice(2));
const runId = args.runId ?? process.env.ENTRANCE_MCP_SMOKE_RUN_ID ?? utcRunId();
const appRoot = resolve(args.appRoot ?? process.env.ENTRANCE_MCP_SMOKE_APP_ROOT ?? resolve(rootDir, `entrance-auto/tmp/mcp-stdio-smoke-${runId}`));
const reportDir = resolve(args.reportDir ?? process.env.ENTRANCE_MCP_SMOKE_REPORT_DIR ?? resolve(rootDir, "entrance-auto/reports"));
const fullGates = args.fullGates ?? false;
const reportJson = resolve(reportDir, `mcp-stdio-smoke-${runId}.json`);
const reportMd = resolve(reportDir, `mcp-stdio-smoke-${runId}.md`);
mkdirSync(appRoot, { recursive: true });
mkdirSync(reportDir, { recursive: true });

if (fullGates) {
  runInSrc("cargo check --workspace", "cargo", ["check", "--workspace"]);
  runInSrc("cargo test --workspace", "cargo", ["test", "--workspace"]);
  runInSrc("pnpm check", "pnpm", ["check"]);
  runInSrc("pnpm build", "pnpm", ["build"]);
  runInSrc("cargo fmt --all --check", "cargo", ["fmt", "--all", "--check"]);
  run("git", ["-C", rootDir, "diff", "--check"]);
}
runInSrc("build entrance debug binary", "cargo", ["build", "-q", "-p", "entrance-app", "--bin", "entrance"]);
const entranceBin = resolve(srcDir, "target/debug/entrance");
if (!existsSync(entranceBin)) throw new Error(`missing entrance binary: ${entranceBin}`);

const sourceCommit = run("git", ["-C", rootDir, "rev-parse", "--short", "HEAD"], { capture: true }).trim();
const observations = {};
const client = startMcpServer(entranceBin, appRoot);
try {
  const initialize = await client.request("initialize", {
    protocolVersion: "2025-06-18",
    capabilities: {},
    clientInfo: { name: "entrance-mcp-stdio-smoke", version: runId },
  });
  assert(initialize.protocolVersion === "2025-06-18", "initialize did not negotiate fallback protocol");
  client.notify("notifications/initialized", {});
  await client.request("ping", {});

  const tools = await client.request("tools/list", {});
  const toolNames = new Set((tools.tools ?? []).map((tool) => tool.name));
  const externalSurfaceTerms = ["mirror", "publish", "readback", "roundtrip", "remote-fixture"];
  for (const name of [
    "entrance_issue_create",
    "entrance_issue_list",
    "entrance_issue_show",
    "entrance_issue_claim",
    "entrance_issue_comment",
    "entrance_issue_run",
    "entrance_issue_advance",
    "entrance_issue_review",
    "entrance_issue_retry",
    "entrance_issue_decide",
    "entrance_issue_control",
    "entrance_loop_create",
    "entrance_loop_control",
    "entrance_review_queue",
  ]) assert(toolNames.has(name), `tools/list missing ${name}`);
  assert(
    ![...toolNames].some((name) => externalSurfaceTerms.some((term) => name.includes(term))),
    "MCP tools still expose external sync surface",
  );

  const prompts = await client.request("prompts/list", {});
  const promptNames = new Set((prompts.prompts ?? []).map((prompt) => prompt.name));
  assert(promptNames.has("entrance_issue_review"), "missing issue review prompt");

  const templates = await client.request("resources/templates/list", {});
  const templateUris = new Set((templates.resourceTemplates ?? []).map((template) => template.uriTemplate));
  assert(templateUris.has("entrance://issues/{issue_id}/control"), "missing issue control template");
  assert(templateUris.has("entrance://issues/{issue_id}/timeline"), "missing issue timeline template");
  assert(templateUris.has("entrance://issues/{issue_id}/transition-policy"), "missing issue transition policy template");
  assert(templateUris.has("entrance://loops/{loop_id}/control"), "missing loop control template");
  assert(templateUris.has("entrance://loops/{loop_id}/dashboard"), "missing loop dashboard template");
  assert(templateUris.has("entrance://loops/{loop_id}/evidence-drilldown"), "missing evidence drilldown template");
  assert(templateUris.has("entrance://loops/{loop_id}/evidence-manifest"), "missing evidence manifest template");
  assert(templateUris.has("entrance://loops/{loop_id}/runtime-preflight"), "missing runtime preflight template");
  assert(templateUris.has("entrance://loops/{loop_id}/worker-lifecycle"), "missing worker lifecycle template");
  assert(
    ![...templateUris].some((uri) => externalSurfaceTerms.some((term) => uri.includes(term))),
    "resource templates still expose external sync surface",
  );

  const create = await callTool(client, "entrance_issue_create", {
    title: "MCP stdio local smoke",
    goal: "Create and run a local issue-bound loop through MCP stdio.",
    runtime: "local",
  });
  const loopId = create.contract?.id ?? create.loop?.id ?? create.loop_id;
  const issueId = create.issues?.[0]?.issue?.id ?? create.issues?.[0]?.id ?? create.issue?.id;
  assert(Number.isInteger(loopId), "create did not return loop id");
  assert(Number.isInteger(issueId), "create did not return issue id");

  const issueBefore = await callTool(client, "entrance_issue_control", { issue_id: issueId });
  assert(issueBefore.schema_version === "entrance.mcp.issue_control.v1", "unexpected issue control schema before run");
  assert(issueBefore.advance_next_action, "issue control did not expose advance_next_action before run");

  const advanceResult = await callTool(client, "entrance_issue_advance", { issue_id: issueId, mode: "until_wait", runtime: "local", max_steps: 3 });
  const runIssueStatus = advanceResult.issue?.issue?.status ?? advanceResult.issue?.status;
  assert(advanceResult.schema_version === "entrance.hive.auto_advance.v1", "unexpected advance schema");
  assert(advanceResult.stop_reason === "done", "advance did not stop at done");
  assert(runIssueStatus === "Done", "issue advance did not end Done");

  const loopControl = await callTool(client, "entrance_loop_control", { loop_id: loopId });
  assert(loopControl.schema_version === "entrance.mcp.loop_control.v1", "unexpected loop control schema");
  assert((loopControl.loop?.id ?? loopControl.loop_id) === loopId, "loop control id mismatch");

  const issueResource = await readJsonResource(client, `entrance://issues/${issueId}/control`);
  assert(issueResource.schema_version === "entrance.mcp.issue_control.v1", "issue control resource schema changed");
  const issueTimeline = await readJsonResource(client, `entrance://issues/${issueId}/timeline`);
  assert(issueTimeline.schema_version === "entrance.hive.issue_timeline.v1", "issue timeline resource schema changed");
  const issueTransition = await readJsonResource(client, `entrance://issues/${issueId}/transition-policy`);
  assert(issueTransition.schema_version === "entrance.hive.issue_transition_policy.v1", "issue transition policy resource schema changed");
  const loopResource = await readJsonResource(client, `entrance://loops/${loopId}/control`);
  assert(loopResource.schema_version === "entrance.mcp.loop_control.v1", "loop control resource schema changed");
  const loopDashboard = await readJsonResource(client, `entrance://loops/${loopId}/dashboard`);
  assert(loopDashboard.schema_version === "entrance.hive.loop_dashboard.v1", "loop dashboard resource schema changed");
  const evidenceDrilldown = await readJsonResource(client, `entrance://loops/${loopId}/evidence-drilldown`);
  assert(evidenceDrilldown.schema_version === "entrance.hive.evidence_drilldown.v1", "evidence drilldown resource schema changed");
  const evidenceManifest = await readJsonResource(client, `entrance://loops/${loopId}/evidence-manifest`);
  assert(evidenceManifest.schema_version === "entrance.hive.evidence_manifest.v1", "evidence manifest resource schema changed");
  const runtimePreflight = await readJsonResource(client, `entrance://loops/${loopId}/runtime-preflight`);
  assert(runtimePreflight.schema_version === "entrance.hive.runtime_preflight.v1", "runtime preflight resource schema changed");
  const workerLifecycle = await readJsonResource(client, `entrance://loops/${loopId}/worker-lifecycle`);
  assert(workerLifecycle.schema_version === "entrance.hive.worker_lifecycle.v1", "worker lifecycle resource schema changed");

  const reviewQueue = await callTool(client, "entrance_review_queue", {});
  assert(reviewQueue.schema_version === "entrance.mcp.review_queue.v1", "review queue schema changed");

  const permissions = await readJsonResource(client, "entrance://policy/mcp-permissions");
  assert(permissions.schema_version === "entrance.mcp.permission_policy.v1", "permission policy schema changed");
  const actor = await readJsonResource(client, "entrance://policy/actor-identity");
  assert(actor.schema_version === "entrance.mcp.actor_identity_policy.v1", "actor policy schema changed");

  const retryRefusal = await client.requestRaw("tools/call", {
    name: "entrance_issue_retry",
    arguments: { issue_id: issueId, human_confirmed: false, body: "should refuse" },
  });
  assert(Boolean(retryRefusal.error), "retry without human confirmation was not refused");

  const resources = await client.request("resources/list", {});
  const resourceUris = new Set((resources.resources ?? []).map((resource) => resource.uri));
  assert(resourceUris.has(`entrance://issues/${issueId}/control`), "resources/list missing issue control");
  assert(resourceUris.has(`entrance://issues/${issueId}/timeline`), "resources/list missing issue timeline");
  assert(resourceUris.has(`entrance://issues/${issueId}/transition-policy`), "resources/list missing transition policy");
  assert(resourceUris.has(`entrance://loops/${loopId}/dashboard`), "resources/list missing loop dashboard");
  assert(resourceUris.has(`entrance://loops/${loopId}/evidence-drilldown`), "resources/list missing evidence drilldown");
  assert(resourceUris.has(`entrance://loops/${loopId}/evidence-manifest`), "resources/list missing evidence manifest");
  assert(resourceUris.has(`entrance://loops/${loopId}/runtime-preflight`), "resources/list missing runtime preflight");
  assert(resourceUris.has(`entrance://loops/${loopId}/worker-lifecycle`), "resources/list missing worker lifecycle");
  assert(
    ![...resourceUris].some((uri) => externalSurfaceTerms.some((term) => uri.includes(term))),
    "resources/list still exposes external sync surface",
  );

  observations.initialize = { protocolVersion: initialize.protocolVersion, server: initialize.serverInfo };
  observations.tools = { count: tools.tools?.length ?? 0, external_sync_surface_absent: true };
  observations.created = { issue_id: issueId, loop_id: loopId };
  observations.run = { issue_status: runIssueStatus, advance_stop_reason: advanceResult.stop_reason };
  observations.resources = { count: resources.resources?.length ?? 0, external_sync_surface_absent: true };
  observations.human_boundary = { retry_without_confirmation_refused: true };
  await client.close();
} catch (error) {
  await client.close().catch(() => undefined);
  throw error;
}

const report = {
  schema_version: "entrance.auto.mcp_stdio_smoke.v2",
  run_id: runId,
  generated_at: new Date().toISOString(),
  source_commit: sourceCommit,
  full_gates: fullGates,
  app_root: appRoot,
  observations,
};
writeFileSync(reportJson, `${JSON.stringify(report, null, 2)}\n`);
writeFileSync(reportMd, [`# MCP Stdio Smoke`, ``, `- Run id: ${runId}`, `- Issue: #${observations.created.issue_id}`, `- Loop: #${observations.created.loop_id}`, `- Tools: ${observations.tools.count}`, `- External sync surface absent: ${observations.tools.external_sync_surface_absent}`, `- Report JSON: \`${reportJson}\``].join("\n") + "\n");
console.log(`Wrote ${reportJson}`);
console.log(`Wrote ${reportMd}`);

function parseArgs(argv) {
  const parsed = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--full-gates") parsed.fullGates = true;
    else if (arg === "--run-id") parsed.runId = argv[++i];
    else if (arg === "--app-root") parsed.appRoot = argv[++i];
    else if (arg === "--report-dir") parsed.reportDir = argv[++i];
    else if (arg === "-h" || arg === "--help") {
      console.log("Usage: run-mcp-stdio-smoke.mjs [--full-gates] [--run-id <id>] [--app-root <path>] [--report-dir <path>]");
      process.exit(0);
    } else throw new Error(`unknown argument: ${arg}`);
  }
  return parsed;
}
function utcRunId() { return new Date().toISOString().replaceAll(/[-:.]/g, "").replace("T", "T").slice(0, 15) + "Z"; }
function runInSrc(label, command, args) { console.error(`==> ${label}`); run(command, args, { cwd: srcDir }); }
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: options.cwd ?? rootDir, env: process.env, encoding: "utf8" });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed\n${result.stdout}\n${result.stderr}`);
  if (!options.capture) {
    if (result.stdout) process.stdout.write(result.stdout);
    if (result.stderr) process.stderr.write(result.stderr);
  }
  return result.stdout ?? "";
}
function assert(condition, message) { if (!condition) throw new Error(message); }
async function callTool(client, name, args) {
  const result = await client.request("tools/call", { name, arguments: args });
  return result.structuredContent ?? JSON.parse(result.content?.[0]?.text ?? "{}");
}
async function readJsonResource(client, uri) {
  const result = await client.request("resources/read", { uri });
  return JSON.parse(result.contents?.[0]?.text ?? "{}");
}
function startMcpServer(binary, appRoot) {
  const child = spawn(binary, ["mcp", "stdio"], { cwd: srcDir, env: { ...process.env, ENTRANCE_APP_ROOT: appRoot }, stdio: ["pipe", "pipe", "pipe"] });
  child.stderr.on("data", (chunk) => process.stderr.write(chunk));
  let nextId = 1;
  const pending = new Map();
  let buffer = "";
  child.stdout.on("data", (chunk) => {
    buffer += chunk.toString("utf8");
    let idx;
    while ((idx = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, idx).trim();
      buffer = buffer.slice(idx + 1);
      if (!line) continue;
      const message = JSON.parse(line);
      const entry = pending.get(message.id);
      if (!entry) continue;
      pending.delete(message.id);
      if (message.error) entry.reject(Object.assign(new Error(message.error.message), { response: message }));
      else entry.resolve(message.result);
    }
  });
  const requestRaw = (method, params) => new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, { resolve: (result) => resolve({ id, result }), reject: (error) => resolve(error.response ?? { id, error: { message: error.message } }) });
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
  });
  const request = async (method, params) => {
    const response = await requestRaw(method, params);
    if (response.error) throw Object.assign(new Error(response.error.message), { response });
    return response.result;
  };
  const notify = (method, params) => child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`);
  const close = async () => { child.kill(); };
  return { request, requestRaw, notify, close };
}
