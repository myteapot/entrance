#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const scriptDir = dirname(__filename);
const rootDir = resolve(scriptDir, "../../..");
const srcDir = resolve(rootDir, "entrance-src");

const defaults = {
  runId: process.env.ENTRANCE_MCP_SMOKE_RUN_ID ?? utcRunId(),
  appRoot: process.env.ENTRANCE_MCP_SMOKE_APP_ROOT ?? null,
  reportDir: process.env.ENTRANCE_MCP_SMOKE_REPORT_DIR ?? resolve(rootDir, "entrance-auto/reports"),
  fullGates: false,
};

const args = parseArgs(process.argv.slice(2));
const runId = args.runId ?? defaults.runId;
const appRoot = resolve(args.appRoot ?? defaults.appRoot ?? resolve(rootDir, `entrance-auto/tmp/mcp-stdio-smoke-${runId}`));
const reportDir = resolve(args.reportDir ?? defaults.reportDir);
const fullGates = args.fullGates ?? defaults.fullGates;
const reportJson = resolve(reportDir, `mcp-stdio-smoke-${runId}.json`);
const reportMd = resolve(reportDir, `mcp-stdio-smoke-${runId}.md`);
const sourceCommit = run("git", ["-C", rootDir, "rev-parse", "--short", "HEAD"], {
  capture: true,
}).trim();

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
if (!existsSync(entranceBin)) {
  throw new Error(`missing entrance binary: ${entranceBin}`);
}

const transcript = [];
const observations = {};
let child = null;

try {
  const client = startMcpServer(entranceBin, appRoot);

  const initialize = await client.request("initialize", {
    protocolVersion: "2025-06-18",
    capabilities: {},
    clientInfo: {
      name: "entrance-mcp-stdio-smoke",
      version: runId,
    },
  });
  assert(initialize.protocolVersion === "2025-06-18", "initialize did not negotiate fallback protocol");
  assert(initialize.capabilities?.tools?.listChanged === false, "initialize missing tools capability");
  assert(initialize.capabilities?.resources?.listChanged === false, "initialize missing resources capability");
  assert(initialize.capabilities?.prompts?.listChanged === false, "initialize missing prompts capability");
  observations.initialize = {
    protocolVersion: initialize.protocolVersion,
    server: initialize.serverInfo,
  };

  client.notify("notifications/initialized", {});
  await client.request("ping", {});

  const tools = await client.request("tools/list", {});
  const toolNames = new Set((tools.tools ?? []).map((tool) => tool.name));
  for (const name of [
    "entrance_loop_create",
    "entrance_issue_run",
    "entrance_issue_control",
    "entrance_loop_control",
    "entrance_review_queue",
    "entrance_connector_control",
    "entrance_connector_queue",
    "entrance_connector_roundtrip_plan",
    "entrance_connector_roundtrip_execute",
  ]) {
    assert(toolNames.has(name), `tools/list missing ${name}`);
  }
  const loopControlTool = tools.tools.find((tool) => tool.name === "entrance_loop_control");
  assert(loopControlTool?.annotations?.entrance_permission?.schema_version === "entrance.mcp.tool_permission.v1", "loop control tool missing permission annotation");
  observations.tools = {
    count: tools.tools?.length ?? 0,
    required_present: true,
    loop_control_access: loopControlTool.annotations.entrance_permission.access,
  };

  const prompts = await client.request("prompts/list", {});
  const promptNames = new Set((prompts.prompts ?? []).map((prompt) => prompt.name));
  for (const name of [
    "entrance_loop_contract",
    "entrance_issue_advance",
    "entrance_loop_review",
    "entrance_connector_decision",
  ]) {
    assert(promptNames.has(name), `prompts/list missing ${name}`);
  }
  observations.prompts = {
    count: prompts.prompts?.length ?? 0,
    required_present: true,
  };

  const templates = await client.request("resources/templates/list", {});
  const templateUris = new Set((templates.resourceTemplates ?? []).map((template) => template.uriTemplate));
  for (const uri of [
    "entrance://issues/{issue_id}/control",
    "entrance://loops/{loop_id}/control",
    "entrance://loops/{loop_id}/evidence-manifest",
  ]) {
    assert(templateUris.has(uri), `resources/templates/list missing ${uri}`);
  }
  observations.resource_templates = {
    count: templates.resourceTemplates?.length ?? 0,
    loop_control_template_present: templateUris.has("entrance://loops/{loop_id}/control"),
  };

  const contractPrompt = await client.request("prompts/get", {
    name: "entrance_loop_contract",
    arguments: {
      goal: "Verify MCP stdio can create and run an observable Entrance loop.",
      boundary: "Use local runtime only and keep all artifacts in the temporary app root.",
      runtime: "local",
    },
  });
  const contractPromptText = promptText(contractPrompt);
  assert(contractPromptText.includes("entrance_loop_create"), "loop contract prompt does not guide loop creation");
  assert(contractPromptText.includes("Developer") && contractPromptText.includes("Reviewer"), "loop contract prompt does not preserve role language");

  const createResult = await callTool(client, "entrance_loop_create", {
    title: "MCP stdio smoke loop",
    goal: "Create a local issue-bound loop through MCP stdio.",
    boundary: "Temporary app root only; local deterministic runtime.",
    runtime: "local",
    review_surface: "local-hive-panel",
    approach_space: [
      "create issue through MCP",
      "run local deterministic worker",
      "inspect loop control packet",
    ],
    eval_space: [
      "MCP tool calls succeed",
      "Reviewer keeps candidate",
      "loop control exposes gates and evidence",
    ],
  });
  assert(createResult.schema_version === "entrance.mcp.loop_create.v1", "unexpected loop create schema");
  const loopId = createResult.loop?.id;
  const issueId = createResult.issues?.[0]?.issue?.id ?? createResult.issues?.[0]?.id;
  assert(Number.isInteger(loopId), "loop create did not return loop id");
  assert(Number.isInteger(issueId), "loop create did not return issue id");
  observations.created = {
    loop_id: loopId,
    issue_id: issueId,
  };

  const issueControlBefore = await callTool(client, "entrance_issue_control", { issue_id: issueId });
  assert(issueControlBefore.schema_version === "entrance.mcp.issue_control.v1", "unexpected issue control schema before run");
  assert(issueControlBefore.state?.status === "Todo", "new MCP issue is not Todo");
  assert(issueControlBefore.resources?.loop_control === `entrance://loops/${loopId}/control`, "issue control missing loop control resource");

  const runResult = await callTool(client, "entrance_issue_run", {
    issue_id: issueId,
    runtime: "local",
  });
  assert(runResult.schema_version === "entrance.mcp.issue_run.v1", "unexpected issue run schema");
  assert(runResult.issues?.[0]?.issue?.status === "Done" || runResult.issues?.[0]?.status === "Done", "MCP issue run did not end Done");
  assert(runResult.verdicts?.[0]?.decision === "keep", "MCP issue run reviewer did not keep candidate");

  const loopControlToolResult = await callTool(client, "entrance_loop_control", { loop_id: loopId });
  assertLoopControl(loopControlToolResult, loopId, issueId);

  const loopControlResource = await readJsonResource(client, `entrance://loops/${loopId}/control`);
  assertLoopControl(loopControlResource, loopId, issueId);

  const issueControlResource = await readJsonResource(client, `entrance://issues/${issueId}/control`);
  assert(issueControlResource.schema_version === "entrance.mcp.issue_control.v1", "issue control resource schema changed");
  assert(issueControlResource.state?.status === "Done", "issue control resource status changed");
  assert(issueControlResource.resources?.loop_control === `entrance://loops/${loopId}/control`, "issue control resource missing loop control pointer");

  const permissions = await readJsonResource(client, "entrance://policy/mcp-permissions");
  assert(permissions.schema_version === "entrance.mcp.permission_policy.v1", "permission policy schema changed");
  const retryConfirmation = (permissions.requires_human_confirmation ?? [])
    .find((item) => item.tool === "entrance_issue_retry");
  assert(retryConfirmation?.argument === "human_confirmed", "permission policy missing retry confirmation");

  const actorIdentity = await readJsonResource(client, "entrance://policy/actor-identity");
  assert(actorIdentity.schema_version === "entrance.mcp.actor_identity_policy.v1", "actor identity policy schema changed");
  const mcpActorBinding = (actorIdentity.bindings ?? []).find((binding) => binding.surface === "mcp");
  assert(actorIdentity.verified_identity?.available === false, "actor identity should not claim verified identity");
  assert(mcpActorBinding?.verified === false, "MCP actor binding should remain explicitly unverified");

  const reviewPrompt = await client.request("prompts/get", {
    name: "entrance_loop_review",
    arguments: { loop_id: loopId },
  });
  const reviewPromptText = promptText(reviewPrompt);
  const reviewPromptResource = promptResourceText(reviewPrompt, `entrance://loops/${loopId}/control`);
  assert(reviewPromptText.includes("Do not implement"), "loop review prompt does not preserve Reviewer boundary");
  assert(reviewPromptText.includes("human_confirmed=true"), "loop review prompt does not preserve human confirmation boundary");
  assert(JSON.parse(reviewPromptResource).schema_version === "entrance.mcp.loop_control.v1", "loop review prompt missing loop control resource");

  const reviewQueue = await callTool(client, "entrance_review_queue", {});
  assert(reviewQueue.schema_version === "entrance.mcp.review_queue.v1", "review queue schema changed");
  assert(reviewQueue.count === 0, "fresh kept loop unexpectedly entered review queue");

  const retryWithoutConfirmation = await client.request("tools/call", {
    name: "entrance_issue_retry",
    arguments: {
      issue_id: issueId,
      body: "This should be refused by MCP smoke.",
      human_confirmed: false,
    },
  });
  assert(retryWithoutConfirmation.isError === true, "retry without human confirmation was not refused");
  assert(
    retryWithoutConfirmation.structuredContent?.error?.includes("human_confirmed=true"),
    "retry refusal did not name human confirmation boundary",
  );

  const remoteReviewSurface = `remote-fixture:ENTRANCE-MCP-${runId.replaceAll(/[^A-Za-z0-9_-]/g, "-")}`;
  const connectorCreateResult = await callTool(client, "entrance_loop_create", {
    title: "MCP stdio remote fixture loop",
    goal: "Create a remote-fixture issue surface through MCP stdio.",
    boundary: "Use the local remote-fixture connector only.",
    runtime: "local",
    review_surface: remoteReviewSurface,
    approach_space: [
      "create remote fixture issue through MCP",
      "read connector queue/control",
      "execute roundtrip after human confirmation",
    ],
    eval_space: [
      "connector control exposes A/B/C options",
      "roundtrip execute requires human_confirmed",
      "remote fixture connector becomes current",
    ],
  });
  const connectorLoopId = connectorCreateResult.loop?.id;
  const connectorIssueId = connectorCreateResult.issues?.[0]?.issue?.id ?? connectorCreateResult.issues?.[0]?.id;
  assert(Number.isInteger(connectorLoopId), "remote fixture create did not return loop id");
  assert(Number.isInteger(connectorIssueId), "remote fixture create did not return issue id");

  const connectorQueueBefore = await callTool(client, "entrance_connector_queue", {
    provider: "remote-fixture",
  });
  assert(connectorQueueBefore.schema_version === "entrance.hive.connector_queue.v1", "connector queue schema changed");
  assert(connectorQueueBefore.provider_filter === "remote-fixture", "connector queue provider filter changed");
  assert(connectorQueueBefore.publish_required_count >= 1, "connector queue did not require publish before roundtrip");
  assert((connectorQueueBefore.issues ?? []).some((issue) => issue.id === connectorIssueId), "connector queue missing remote fixture issue");

  const connectorControlBefore = await callTool(client, "entrance_connector_control", {
    provider: "remote-fixture",
  });
  assertConnectorControlReady(connectorControlBefore);

  const connectorDecisionPrompt = await client.request("prompts/get", {
    name: "entrance_connector_decision",
    arguments: { provider: "remote-fixture" },
  });
  const connectorDecisionPromptText = promptText(connectorDecisionPrompt);
  const connectorDecisionPromptResource = promptResourceText(connectorDecisionPrompt, "entrance://connectors/control/remote-fixture");
  assert(connectorDecisionPromptText.includes("human_confirmed=true"), "connector decision prompt missing human confirmation boundary");
  assert(connectorDecisionPromptText.includes("plan_id"), "connector decision prompt missing plan_id boundary");
  assert(JSON.parse(connectorDecisionPromptResource).schema_version === "entrance.mcp.connector_control.v1", "connector decision prompt missing connector control resource");

  const roundtripPlan = await callTool(client, "entrance_connector_roundtrip_plan", {
    provider: "remote-fixture",
  });
  assert(roundtripPlan.schema_version === "entrance.hive.connector_roundtrip_plan.v1", "connector roundtrip plan schema changed");
  assert(roundtripPlan.provider_filter === "remote-fixture", "roundtrip plan provider filter changed");
  assert(roundtripPlan.can_execute === true, "roundtrip plan was not executable");
  assert(typeof roundtripPlan.plan_id === "string" && roundtripPlan.plan_id.length >= 16, "roundtrip plan missing plan_id");
  assert((roundtripPlan.issues ?? []).some((issue) => issue.id === connectorIssueId), "roundtrip plan missing remote fixture issue");

  const connectorExecuteWithoutConfirmation = await client.request("tools/call", {
    name: "entrance_connector_roundtrip_execute",
    arguments: {
      provider: "remote-fixture",
      plan_id: roundtripPlan.plan_id,
      human_confirmed: false,
    },
  });
  assert(connectorExecuteWithoutConfirmation.isError === true, "connector roundtrip without human confirmation was not refused");
  assert(
    connectorExecuteWithoutConfirmation.structuredContent?.error?.includes("human_confirmed=true"),
    "connector roundtrip refusal did not name human confirmation boundary",
  );

  const roundtripExecute = await callTool(client, "entrance_connector_roundtrip_execute", {
    provider: "remote-fixture",
    plan_id: roundtripPlan.plan_id,
    human_confirmed: true,
    author: "mcp-smoke-human",
  });
  assert(roundtripExecute.schema_version === "entrance.hive.connector_roundtrip_execute.v1", "connector roundtrip execute schema changed");
  assert(roundtripExecute.reason === "plan_executed", "connector roundtrip execute reason changed");
  assert(roundtripExecute.completed_count >= 1, "connector roundtrip did not complete an issue");
  const connectorRecord = (roundtripExecute.recorded ?? []).find((record) => record.issue_id === connectorIssueId);
  assert(connectorRecord, "connector roundtrip did not record remote fixture issue");
  assert(connectorRecord.operator_confirmation_receipt?.client?.name === "entrance-mcp-stdio-smoke", "connector roundtrip missing MCP client confirmation receipt");
  assert(connectorRecord.operator_confirmation_receipt?.actor?.verified === false, "connector roundtrip receipt should remain explicitly unverified");

  const connectorQueueAfter = await callTool(client, "entrance_connector_queue", {
    provider: "remote-fixture",
  });
  assert(connectorQueueAfter.current_count >= 1, "connector queue did not become current after roundtrip");
  assert(connectorQueueAfter.publish_required_count === 0, "connector queue still requires publish after roundtrip");

  const connectorControlAfter = await readJsonResource(client, "entrance://connectors/control/remote-fixture");
  assert(connectorControlAfter.schema_version === "entrance.mcp.connector_control.v1", "connector control resource schema changed");
  assert(connectorControlAfter.state?.current_count >= 1, "connector control resource did not show current issue");
  assert(connectorControlAfter.state?.publish_required_count === 0, "connector control resource still shows publish required");

  const resources = await client.request("resources/list", {});
  const resourceUris = new Set((resources.resources ?? []).map((resource) => resource.uri));
  assert(resourceUris.has(`entrance://issues/${issueId}/control`), "resources/list missing issue control after run");
  assert(resourceUris.has(`entrance://loops/${loopId}/control`), "resources/list missing loop control after run");
  assert(resourceUris.has(`entrance://issues/${connectorIssueId}/control`), "resources/list missing remote fixture issue control");
  assert(resourceUris.has("entrance://connectors/control/remote-fixture"), "resources/list missing remote-fixture connector control");

  observations.loop_control = {
    schema_version: loopControlToolResult.schema_version,
    issue_status: loopControlToolResult.state.issue_status,
    loop_status: loopControlToolResult.state.loop_status,
    reviewer_decision: loopControlToolResult.state.reviewer_decision,
    reviewer_budget: `${loopControlToolResult.state.reviewer_invalid_rounds_used}/${loopControlToolResult.state.reviewer_invalid_round_budget}`,
    score_names: (loopControlToolResult.reviewer_gate_surface.score_vector ?? []).map((item) => item.name),
    option_keys: (loopControlToolResult.operator_decision_surface.options ?? []).map((option) => option.key),
  };
  observations.resources = {
    count: resources.resources?.length ?? 0,
    loop_control_resource_present: true,
    issue_control_resource_present: true,
  };
  observations.human_boundary = {
    retry_without_confirmation_refused: true,
    connector_roundtrip_without_confirmation_refused: true,
    permission_policy_schema: permissions.schema_version,
    actor_identity_verified: mcpActorBinding?.verified,
  };
  observations.connector = {
    provider: "remote-fixture",
    review_surface: remoteReviewSurface,
    issue_id: connectorIssueId,
    loop_id: connectorLoopId,
    queue_before: {
      publish_required_count: connectorQueueBefore.publish_required_count,
      current_count: connectorQueueBefore.current_count,
    },
    control_before: {
      schema_version: connectorControlBefore.schema_version,
      primary_action: connectorControlBefore.state?.primary_action,
      publish_can_execute: connectorControlBefore.state?.publish_can_execute,
      roundtrip_can_execute: connectorControlBefore.state?.roundtrip_can_execute,
      option_keys: (connectorControlBefore.operator_decision_surface?.options ?? []).map((option) => option.key),
    },
    roundtrip: {
      plan_id: roundtripPlan.plan_id,
      issue_count: roundtripPlan.issue_count,
      completed_count: roundtripExecute.completed_count,
      receipt_client: connectorRecord.operator_confirmation_receipt?.client?.name,
      receipt_actor_verified: connectorRecord.operator_confirmation_receipt?.actor?.verified,
    },
    queue_after: {
      publish_required_count: connectorQueueAfter.publish_required_count,
      current_count: connectorQueueAfter.current_count,
    },
  };

  await client.close();

  const report = {
    schema_version: "entrance.auto.mcp_stdio_smoke.v1",
    run_id: runId,
    generated_at: new Date().toISOString(),
    source_commit: sourceCommit,
    full_gates: fullGates,
    app_root: appRoot,
    observations,
    transcript,
  };
  writeFileSync(reportJson, `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(reportMd, mcpSmokeSummary(report));

  console.log("Entrance MCP stdio smoke validated.");
  console.log(`Report JSON: ${reportJson}`);
  console.log(`Report Markdown: ${reportMd}`);
  console.log(`App root: ${appRoot}`);
} catch (error) {
  if (child) {
    child.kill();
  }
  throw error;
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
  entrance-auto/workflows/validation/run-mcp-stdio-smoke.mjs [--full-gates] [--run-id <id>] [--app-root <path>] [--report-dir <path>]

Starts entrance mcp stdio from a clean app root and drives the local MCP
JSON-RPC protocol as a client. The smoke creates and runs a local issue-bound
loop through MCP tools, executes a confirmed remote-fixture connector roundtrip,
then verifies resources, prompts, permissions, loop control, connector control,
and human-confirmation refusal behavior.`);
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

function runInSrc(name, command, args) {
  process.stderr.write(`==> ${name}\n`);
  run(command, args, { cwd: srcDir });
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? rootDir,
    env: options.env ?? process.env,
    encoding: "utf8",
    stdio: options.capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with exit code ${result.status}`);
  }
  return result.stdout ?? "";
}

function startMcpServer(entranceBin, root) {
  child = spawn(entranceBin, ["mcp", "stdio"], {
    cwd: srcDir,
    env: {
      ...process.env,
      ENTRANCE_APP_ROOT: root,
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");

  let nextId = 1;
  let buffer = "";
  const pending = new Map();
  const stderr = [];

  child.stdout.on("data", (chunk) => {
    buffer += chunk;
    let newline = buffer.indexOf("\n");
    while (newline !== -1) {
      const line = buffer.slice(0, newline).trim();
      buffer = buffer.slice(newline + 1);
      if (line) {
        handleResponseLine(line, pending);
      }
      newline = buffer.indexOf("\n");
    }
  });
  child.stderr.on("data", (chunk) => {
    stderr.push(chunk);
    process.stderr.write(`[mcp] ${chunk}`);
  });
  child.on("exit", (code, signal) => {
    for (const { reject } of pending.values()) {
      reject(new Error(`MCP server exited before response (code=${code}, signal=${signal})`));
    }
    pending.clear();
  });

  return {
    request(method, params = {}) {
      const id = nextId;
      nextId += 1;
      const payload = { jsonrpc: "2.0", id, method, params };
      transcript.push({ direction: "send", id, method, params: sanitizeTranscriptParams(method, params) });
      child.stdin.write(`${JSON.stringify(payload)}\n`);
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`timed out waiting for MCP response to ${method}; stderr=${stderr.join("").slice(-1000)}`));
        }, 30_000);
        pending.set(id, {
          method,
          resolve: (value) => {
            clearTimeout(timeout);
            resolve(value);
          },
          reject: (error) => {
            clearTimeout(timeout);
            reject(error);
          },
        });
      });
    },
    notify(method, params = {}) {
      transcript.push({ direction: "send", method, notification: true });
      child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`);
    },
    close() {
      return new Promise((resolveClose) => {
        if (!child || child.exitCode !== null || child.signalCode !== null) {
          resolveClose();
          return;
        }
        child.once("exit", () => resolveClose());
        child.stdin.end();
        setTimeout(() => {
          if (child && child.exitCode === null && child.signalCode === null) {
            child.kill();
          }
        }, 1000).unref();
      });
    },
  };
}

function handleResponseLine(line, pending) {
  let response;
  try {
    response = JSON.parse(line);
  } catch (error) {
    throw new Error(`invalid MCP JSON response: ${line}; ${error}`);
  }
  transcript.push({
    direction: "recv",
    id: response.id,
    error: response.error?.message ?? null,
    result_keys: response.result ? Object.keys(response.result) : [],
  });
  const waiter = pending.get(response.id);
  if (!waiter) {
    return;
  }
  pending.delete(response.id);
  if (response.error) {
    waiter.reject(new Error(`${waiter.method} returned JSON-RPC error: ${response.error.message}`));
    return;
  }
  waiter.resolve(response.result);
}

async function callTool(client, name, argumentsValue) {
  const result = await client.request("tools/call", {
    name,
    arguments: argumentsValue,
  });
  assert(result.isError !== true, `${name} returned tool error: ${result.content?.[0]?.text}`);
  return result.structuredContent;
}

async function readJsonResource(client, uri) {
  const result = await client.request("resources/read", { uri });
  const text = result.contents?.[0]?.text;
  assert(typeof text === "string", `resource ${uri} did not return text content`);
  return JSON.parse(text);
}

function promptText(promptResult) {
  return (promptResult.messages ?? [])
    .map((message) => message.content?.text ?? "")
    .join("\n");
}

function promptResourceText(promptResult, uri) {
  const message = (promptResult.messages ?? []).find((item) => item.content?.resource?.uri === uri);
  const text = message?.content?.resource?.text;
  assert(typeof text === "string", `prompt missing resource ${uri}`);
  return text;
}

function assertLoopControl(value, loopId, issueId) {
  assert(value.schema_version === "entrance.mcp.loop_control.v1", "loop control schema changed");
  assert(value.loop_id === loopId, "loop control loop id changed");
  assert(value.state?.issue_id === issueId, "loop control issue id changed");
  assert(value.state?.issue_status === "Done", "loop control issue status changed");
  assert(value.state?.loop_status === "kept", "loop control loop status changed");
  assert(value.state?.reviewer_decision === "keep", "loop control reviewer decision changed");
  assert(value.state?.reviewer_invalid_rounds_used === 0, "loop control reviewer invalid rounds changed");
  assert(value.state?.reviewer_invalid_round_budget === 3, "loop control reviewer budget changed");
  assert(value.reviewer_gate_surface?.gates?.runtime_preflight?.state === "admitted", "loop control runtime gate changed");
  assert(value.reviewer_gate_surface?.gates?.worker_lifecycle?.state === "succeeded", "loop control lifecycle gate changed");
  assert(value.reviewer_gate_surface?.gates?.evidence_manifest?.state === "ok", "loop control evidence gate changed");
  const scoreNames = new Set((value.reviewer_gate_surface?.score_vector ?? []).map((item) => item.name));
  for (const score of ["stage_completeness", "runtime_readiness", "evidence_presence", "admission_integrity"]) {
    assert(scoreNames.has(score), `loop control missing score ${score}`);
  }
  const optionKeys = new Set((value.operator_decision_surface?.options ?? []).map((option) => option.key));
  for (const key of ["A", "B", "C"]) {
    assert(optionKeys.has(key), `loop control missing option ${key}`);
  }
  assert(value.human_decision_boundary?.confirmation_arg === "human_confirmed", "loop control missing human confirmation arg");
}

function assertConnectorControlReady(value) {
  assert(value.schema_version === "entrance.mcp.connector_control.v1", "connector control schema changed");
  assert(value.provider_filter === "remote-fixture", "connector control provider filter changed");
  assert(value.state?.provider_known === true, "connector control provider is not known");
  assert(value.state?.publish_required_count >= 1, "connector control did not show publish-required issue");
  assert(value.state?.needs_human_decision === true, "connector control did not expose human decision boundary");
  assert(value.state?.roundtrip_can_execute === true, "connector control roundtrip is not executable");
  assert(value.human_decision_boundary?.required === true, "connector control missing human decision boundary");
  assert(value.human_decision_boundary?.confirmation_arg === "human_confirmed", "connector control missing confirmation arg");
  assert(value.human_decision_boundary?.plan_id_arg === "plan_id", "connector control missing plan_id arg");
  const optionKeys = new Set((value.operator_decision_surface?.options ?? []).map((option) => option.key));
  for (const key of ["A", "B", "C"]) {
    assert(optionKeys.has(key), `connector control missing option ${key}`);
  }
  const roundtripOption = (value.operator_decision_surface?.options ?? []).find((option) => option.key === "B");
  assert(roundtripOption?.enabled === true, "connector control roundtrip option is not enabled");
  assert(roundtripOption?.tool === "entrance_connector_roundtrip_execute", "connector control roundtrip option tool changed");
  assert(typeof roundtripOption?.plan_id === "string" && roundtripOption.plan_id.length >= 16, "connector control roundtrip option missing plan_id");
}

function sanitizeTranscriptParams(method, params) {
  if (method === "tools/call") {
    return {
      name: params.name,
      argument_keys: Object.keys(params.arguments ?? {}),
    };
  }
  if (method === "prompts/get") {
    return {
      name: params.name,
      argument_keys: Object.keys(params.arguments ?? {}),
    };
  }
  if (method === "resources/read") {
    return { uri: params.uri };
  }
  return params;
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function mcpSmokeSummary(report) {
  return [
    "# Entrance MCP Stdio Smoke Report",
    "",
    `- Run id: ${report.run_id}`,
    `- Source commit: ${report.source_commit}`,
    `- App root: \`${report.app_root}\``,
    `- Full gates: ${report.full_gates ? "yes" : "no"}`,
    `- Protocol: ${report.observations.initialize.protocolVersion}`,
    `- Created issue: #${report.observations.created.issue_id}`,
    `- Created loop: #${report.observations.created.loop_id}`,
    "",
    "## Loop Control",
    "",
    `- Schema: ${report.observations.loop_control.schema_version}`,
    `- Issue status: ${report.observations.loop_control.issue_status}`,
    `- Loop status: ${report.observations.loop_control.loop_status}`,
    `- Reviewer decision: ${report.observations.loop_control.reviewer_decision}`,
    `- Reviewer budget: ${report.observations.loop_control.reviewer_budget}`,
    `- Score names: ${report.observations.loop_control.score_names.join(", ")}`,
    `- Operator options: ${report.observations.loop_control.option_keys.join(", ")}`,
    "",
    "## Connector Control",
    "",
    `- Provider: ${report.observations.connector.provider}`,
    `- Remote issue: #${report.observations.connector.issue_id}`,
    `- Publish required before: ${report.observations.connector.queue_before.publish_required_count}`,
    `- Roundtrip executable: ${report.observations.connector.control_before.roundtrip_can_execute}`,
    `- Roundtrip completed: ${report.observations.connector.roundtrip.completed_count}/${report.observations.connector.roundtrip.issue_count}`,
    `- Publish required after: ${report.observations.connector.queue_after.publish_required_count}`,
    `- Connector options: ${report.observations.connector.control_before.option_keys.join(", ")}`,
    "",
    "## Human Boundary",
    "",
    `- Retry without confirmation refused: ${report.observations.human_boundary.retry_without_confirmation_refused}`,
    `- Connector roundtrip without confirmation refused: ${report.observations.human_boundary.connector_roundtrip_without_confirmation_refused}`,
    `- Permission policy: ${report.observations.human_boundary.permission_policy_schema}`,
    `- Actor identity verified: ${report.observations.human_boundary.actor_identity_verified}`,
    "",
  ].join("\n");
}
