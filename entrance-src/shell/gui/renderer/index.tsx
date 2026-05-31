/* @refresh reload */
import { render } from "solid-js/web";
import App from "./App";
import "./styles/theme.css";
import "./styles/global.css";
import "./styles/app.css";

const detectPlatform = (): "windows" | "mac" | "linux" | "unknown" => {
  if (typeof navigator === "undefined") {
    return "unknown";
  }

  const userAgent = navigator.userAgent ?? "";
  if (/windows/i.test(userAgent)) {
    return "windows";
  }
  if (/macintosh|mac os x/i.test(userAgent)) {
    return "mac";
  }
  if (/linux/i.test(userAgent)) {
    return "linux";
  }

  return "unknown";
};

document.documentElement.dataset.platform = detectPlatform();

render(() => <App />, document.getElementById("root") as HTMLElement);
