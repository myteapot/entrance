/* @refresh reload */
import { render } from "solid-js/web";
import "./styles/theme.css";
import "./styles/global.css";
import "./styles/titlebar.css";
import App from "./App";

render(() => <App />, document.getElementById("root") as HTMLElement);
