import type { ParentComponent } from "solid-js";

const MainPanel: ParentComponent = (props) => {
  return (
    <main class="main-panel">
      <div class="main-panel__inner">{props.children}</div>
    </main>
  );
};

export default MainPanel;
