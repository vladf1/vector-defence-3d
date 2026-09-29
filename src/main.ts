import "./style.css";
import { mount } from "svelte";
import App from "./App.svelte";
import { selectStartupGameProfile } from "./game-profile";
import { loadEngine } from "./engine";
import { prefetchGpuDevice } from "./gpu-device";

const target = document.querySelector<HTMLDivElement>("#app");

if (!target) {
  throw new Error("Missing app root.");
}

function updateAppViewportHeight(): void {
  const viewportHeight = window.visualViewport?.height ?? window.innerHeight;
  document.documentElement.style.setProperty("--app-viewport-height", `${viewportHeight}px`);
}

updateAppViewportHeight();
window.addEventListener("resize", updateAppViewportHeight);
window.visualViewport?.addEventListener("resize", updateAppViewportHeight);
window.visualViewport?.addEventListener("scroll", updateAppViewportHeight);

// Start acquiring the GPU and compiling the engine while Svelte mounts; the session reuses
// both and reports any failure.
prefetchGpuDevice();
loadEngine().catch(() => undefined);

mount(App, {
  target,
  props: {
    profile: selectStartupGameProfile(window),
  },
});
