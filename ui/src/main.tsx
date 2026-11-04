import { render } from "solid-js/web";

import { App } from "./app.tsx";
import "./styles.css";

const root = document.getElementById("app");
if (root === null) {
  throw new Error("#app mount point missing — index.html out of sync with main.tsx");
}
render(() => <App />, root);
