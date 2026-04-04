import { A } from "@solidjs/router";
import { primaryRoutes, settingsRoute } from "../router";

const Sidebar = () => {
  const availableHotkeys = primaryRoutes
    .map((route) => route.hotkey)
    .filter((hotkey): hotkey is string => Boolean(hotkey));
  const hotkeyHint =
    availableHotkeys.length > 1
      ? `Quick switch with ${availableHotkeys[0]} to ${availableHotkeys[availableHotkeys.length - 1]}.`
      : "Quick switch from the keyboard.";

  return (
    <aside class="sidebar">
      <div class="sidebar__brand">
        <div class="sidebar__brand-mark">EN</div>
        <div>
          <p class="sidebar__eyebrow">Entrance</p>
          <h1 class="sidebar__title">Native NOTA front door</h1>
          <p class="sidebar__summary">
            Chat opens on live runtime truth, Board visualizes the active mission boundary, and
            Do remains the bounded action lane for automatic transactions.
          </p>
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
        <p class="sidebar__hint">{hotkeyHint}</p>
      </div>
    </aside>
  );
};

export default Sidebar;
