import { invoke } from "@tauri-apps/api/core";

export interface DashboardSummary {
  app_version: string;
  launcher_hotkey: string | null;
  enabled_plugin_count: number;
  running_task_count: number;
  last_activity_at: string | null;
  token_count: number;
  mcp_config_count: number;
  enabled_mcp_count: number;
}

export const fetchDashboardSummary = () => invoke<DashboardSummary>("dashboard_summary");
