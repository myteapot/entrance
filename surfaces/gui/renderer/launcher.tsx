/* @refresh reload */
import { render } from "solid-js/web";
import "./styles/theme.css";
import "./styles/global.css";
import LauncherWindow from "./LauncherWindow";

render(() => <LauncherWindow />, document.getElementById("root") as HTMLElement);
