import type { Component } from "solid-js";

type RoutePlaceholderProps = {
  title: string;
  description: string;
  path: string;
};

const RoutePlaceholder: Component<RoutePlaceholderProps> = (props) => {
  return (
    <section class="page page--placeholder">
      <header class="page__hero">
        <p class="page__eyebrow">Route shell</p>
        <h2>{props.title}</h2>
        <p class="page__summary">{props.description}</p>
      </header>

      <div class="placeholder-panel">
        <div>
          <p class="placeholder-panel__label">Route</p>
          <code>{props.path}</code>
        </div>
        <p>
          This screen is intentionally kept lightweight so future issues can fill it with the real module UI without
          changing the surrounding navigation or layout shell.
        </p>
      </div>
    </section>
  );
};

export default RoutePlaceholder;
