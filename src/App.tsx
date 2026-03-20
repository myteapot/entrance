import { For, onCleanup, onMount, type Component } from "solid-js";
import { Route, Router, type RouteSectionProps, useNavigate } from "@solidjs/router";
import "./App.css";
import MainPanel from "./components/MainPanel";
import Sidebar from "./components/Sidebar";
import { appRoutes, shortcutRoutes } from "./router";

const isEditableTarget = (target: EventTarget | null) => {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  const tagName = target.tagName.toLowerCase();

  return (
    tagName === "input" ||
    tagName === "textarea" ||
    tagName === "select" ||
    target.isContentEditable
  );
};

const AppShell: Component<RouteSectionProps> = (props) => {
  const navigate = useNavigate();

  onMount(() => {
    const handleKeydown = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) {
        return;
      }

      if (isEditableTarget(event.target)) {
        return;
      }

      const shortcutIndex = Number(event.key) - 1;
      const route = shortcutRoutes[shortcutIndex];

      if (!route) {
        return;
      }

      event.preventDefault();
      navigate(route.path);
    };

    window.addEventListener("keydown", handleKeydown);
    onCleanup(() => window.removeEventListener("keydown", handleKeydown));
  });

  return (
    <div class="app-shell">
      <Sidebar />
      <MainPanel>{props.children}</MainPanel>
    </div>
  );
};

function App() {
  return (
    <Router root={AppShell}>
      <For each={appRoutes}>
        {(route) => <Route path={route.path} component={route.component} />}
      </For>
    </Router>
  );
}

export default App;
