import type { Component } from "solid-js";
import Dashboard from "./pages/Dashboard";
import RoutePlaceholder from "./pages/RoutePlaceholder";
import Vault from "./pages/Vault";
import Forge from "./pages/Forge";

type AppRoute = {
  path: string;
  label: string;
  glyph: string;
  description: string;
  hotkey?: string;
  component: Component;
};

const createPlaceholderPage = (title: string, description: string, path: string): Component => {
  const PlaceholderPage: Component = () =>
    RoutePlaceholder({
      title,
      description,
      path,
    });

  return PlaceholderPage;
};

export const primaryRoutes: AppRoute[] = [
  {
    path: "/",
    label: "Dashboard",
    glyph: "DB",
    description: "Overview and widget staging area",
    hotkey: "Ctrl+1",
    component: Dashboard,
  },
  {
    path: "/launcher",
    label: "Launcher",
    glyph: "LN",
    description: "Command launch surface placeholder",
    hotkey: "Ctrl+2",
    component: createPlaceholderPage(
      "Launcher",
      "Launcher stays as a routed placeholder in this slice. The floating interaction model will land in a later issue.",
      "/launcher",
    ),
  },
  {
    path: "/forge",
    label: "Forge",
    glyph: "FG",
    description: "Task runner and execution engine",
    hotkey: "Ctrl+3",
    component: Forge,
  },
  {
    path: "/vault",
    label: "Vault",
    glyph: "VT",
    description: "API Tokens and MCP Configurations",
    hotkey: "Ctrl+4",
    component: Vault,
  },
  {
    path: "/board",
    label: "Board",
    glyph: "BD",
    description: "Planning board placeholder",
    hotkey: "Ctrl+5",
    component: createPlaceholderPage(
      "Board",
      "Board currently exists as a routed shell. Future work can focus purely on task content and interaction design.",
      "/board",
    ),
  },
  {
    path: "/connector",
    label: "Connector",
    glyph: "CN",
    description: "External bridge placeholder",
    hotkey: "Ctrl+6",
    component: createPlaceholderPage(
      "Connector",
      "Connector replaces the earlier Comm route and reserves space for bridge status, sessions, and sync diagnostics.",
      "/connector",
    ),
  },
];

export const settingsRoute: AppRoute = {
  path: "/settings",
  label: "Settings",
  glyph: "ST",
  description: "Workspace preferences placeholder",
  component: createPlaceholderPage(
    "Settings",
    "Settings lives in the footer as a dedicated utility route, separate from the six primary shortcut destinations.",
    "/settings",
  ),
};

export const appRoutes = [...primaryRoutes, settingsRoute];

export const shortcutRoutes = primaryRoutes.filter((route) => route.hotkey);
