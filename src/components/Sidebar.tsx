import { A } from "@solidjs/router";
import { primaryRoutes, settingsRoute } from "../router";

const Sidebar = () => {
  return (
    <aside class="sidebar">
      <div class="sidebar__brand">
        <div class="sidebar__brand-mark">EN</div>
        <div>
          <p class="sidebar__eyebrow">Entrance</p>
          <h1 class="sidebar__title">NOTA host</h1>
          <p class="sidebar__summary">Single-ingress shell with Chat for continuity and Do for automatic runtime transactions.</p>
        </div>
      </div>

      <nav class="sidebar__nav" aria-label="Primary navigation">
        {primaryRoutes.map((route) => (
          <A href={route.path} class="sidebar__link" activeClass="is-active" end>
            <span class="sidebar__glyph" aria-hidden="true">
              {route.glyph}
            </span>
            <span class="sidebar__copy">
              <span class="sidebar__label">{route.label}</span>
              <span class="sidebar__detail">{route.description}</span>
            </span>
            <span class="sidebar__hotkey">{route.hotkey}</span>
          </A>
        ))}
      </nav>

      <div class="sidebar__footer">
        <A href={settingsRoute.path} class="sidebar__link sidebar__link--settings" activeClass="is-active" end>
          <span class="sidebar__glyph" aria-hidden="true">
            {settingsRoute.glyph}
          </span>
          <span class="sidebar__copy">
            <span class="sidebar__label">{settingsRoute.label}</span>
            <span class="sidebar__detail">{settingsRoute.description}</span>
          </span>
        </A>
        <p class="sidebar__hint">Quick switch with Ctrl+1 and Ctrl+2.</p>
      </div>
    </aside>
  );
};

export default Sidebar;
