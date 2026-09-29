<script lang="ts">
  import GameBoard from "./components/GameBoard.svelte";
  import GameModal from "./components/GameModal.svelte";
  import HelpDialog from "./components/HelpDialog.svelte";
  import NerdStatsPanel from "./components/NerdStatsPanel.svelte";
  import TopBar from "./components/TopBar.svelte";
  import TowerDock from "./components/TowerDock.svelte";
  import { untrack } from "svelte";
  import { setGameSessionContext } from "./game-context";
  import type { GameProfile } from "./game-profile";
  import { createGameSession } from "./game-session";

  // Breathing room between the HUD and the framed field, in CSS pixels. The top needs more:
  // the spawn gate and towers on the field's far edge stand up into the view.
  const FIELD_TOP_GAP = 30;
  const FIELD_BOTTOM_GAP = 12;
  const FIELD_SIDE_GAP = 14;

  const { profile }: { profile: GameProfile } = $props();
  const session = untrack(() => createGameSession(profile));
  const nerdStatsVisible = session.nerdStatsVisible;
  let app: HTMLDivElement | undefined = $state();
  let topBar: HTMLElement | undefined = $state();
  let dock: HTMLElement | undefined = $state();

  setGameSessionContext(session);

  /** The canvas fills the screen; the camera frames the field between the top bar and the dock. */
  function syncViewInsets(): void {
    if (!app) {
      return;
    }
    const frame = app.getBoundingClientRect();
    const top = topBar ? topBar.getBoundingClientRect().bottom - frame.top + FIELD_TOP_GAP : 0;
    const bottom = dock && dock.offsetParent !== null ? frame.bottom - dock.getBoundingClientRect().top + FIELD_BOTTOM_GAP : 0;
    // Floating HUD pieces (banner, nerd stats, placing hint) anchor to the same bands.
    app.style.setProperty("--hud-top", `${top - FIELD_TOP_GAP + FIELD_BOTTOM_GAP}px`);
    app.style.setProperty("--hud-bottom", `${bottom}px`);
    session.setViewInsets(top, FIELD_SIDE_GAP, bottom, FIELD_SIDE_GAP);
  }

  $effect(() => {
    const observed = [app, topBar, dock].filter((element): element is HTMLElement => element !== undefined);
    const observer = new ResizeObserver(syncViewInsets);
    for (const element of observed) {
      observer.observe(element);
    }
    syncViewInsets();
    return () => observer.disconnect();
  });
</script>

<svelte:window onresize={syncViewInsets} />

<div class={`app ${profile.mode === "mobile" ? "mobile" : "desktop"}`} bind:this={app}>
  {#if profile.ui.portraitOnly}
    <div class="orientation-blocker">
      <strong>Rotate to portrait</strong>
      <span>Vector Defence mobile is tuned for upright play.</span>
    </div>
  {/if}
  <GameBoard />
  <TopBar bind:element={topBar} />
  <TowerDock bind:element={dock} />
  {#if $nerdStatsVisible}
    <NerdStatsPanel />
  {/if}
  <GameModal />
  <HelpDialog />
</div>
