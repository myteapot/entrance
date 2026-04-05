import type { Component } from "solid-js";
import Chat from "./pages/Chat";
import Dashboard from "./pages/Dashboard";
import Console from "./pages/Console";
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
    label: "Chat",
    glyph: "CH",
    description: "Native front door and current state",
    hotkey: "Ctrl+1",
    component: Chat,
  },
  {
    path: "/do",
    label: "Do",
    glyph: "DO",
    description: "Automatic runtime transaction and dispatch",
    hotkey: "Ctrl+2",
    component: Forge,
  },
  {
    path: "/board",
    label: "Board",
    glyph: "BD",
    description: "Mission dashboard and layered progress",
    hotkey: "Ctrl+3",
    component: Dashboard,
  },
  {
    path: "/console",
    label: "Console",
    glyph: "CS",
    description: "Instance management and system health",
    hotkey: "Ctrl+4",
    component: Console,
  },
];

export const settingsRoute: AppRoute = {
  path: "/settings",
  label: "Settings",
  glyph: "ST",
  description: "Vault, MCP, and updater configuration",
  component: Vault,
};

const hiddenRoutes: AppRoute[] = [
  {
    path: "/forge",
    label: "Do",
    glyph: "DO",
    description: "Legacy alias for Do",
    component: Forge,
  },
  {
    path: "/vault",
    label: "Settings",
    glyph: "ST",
    description: "Legacy alias for Settings",
    component: Vault,
  },
  {
    path: "/launcher",
    label: "Launcher",
    glyph: "LN",
    description: "Command launch surface placeholder",
    component: createPlaceholderPage(
      "Launcher",
      "Launcher stays as a routed placeholder in this slice. The floating interaction model will land in a later issue.",
      "/launcher",
    ),
  },
  {
    path: "/connector",
    label: "Connector",
    glyph: "CN",
    description: "External bridge placeholder",
    component: createPlaceholderPage(
      "Connector",
      "Connector replaces the earlier Comm route and reserves space for bridge status, sessions, and sync diagnostics.",
      "/connector",
    ),
  },
];

export const appRoutes = [...primaryRoutes, settingsRoute, ...hiddenRoutes];

export const shortcutRoutes = primaryRoutes.filter((route) => route.hotkey);
