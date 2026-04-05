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
        <h1 class="sidebar__title">Entrance</h1>
      </div>

      <nav class="sidebar__nav" aria-label="Primary navigation">
        {primaryRoutes.map((route) => (
          <A href={route.path} class="sidebar__link" activeClass="is-active" end>
            <span class="sidebar__label">{route.label}</span>
            <span class="sidebar__hotkey">{route.hotkey}</span>
          </A>
        ))}
      </nav>

      <div class="sidebar__footer">
        <A href={settingsRoute.path} class="sidebar__link sidebar__link--settings" activeClass="is-active" end>
          <span class="sidebar__label">{settingsRoute.label}</span>
        </A>
        <p class="sidebar__hint">{hotkeyHint}</p>
      </div>
    </aside>
  );
};

export default Sidebar;
