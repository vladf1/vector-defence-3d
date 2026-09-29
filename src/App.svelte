<script lang="ts">
  import ChromeBar from "./components/ChromeBar.svelte";
  import GameBoard from "./components/GameBoard.svelte";
  import NerdStatsPanel from "./components/NerdStatsPanel.svelte";
  import TowerPanel from "./components/TowerPanel.svelte";
  import { untrack } from "svelte";
  import { setGameSessionContext } from "./game-context";
  import type { GameProfile } from "./game-profile";
  import { createGameSession } from "./game-session";

  const { profile }: { profile: GameProfile } = $props();
  const session = untrack(() => createGameSession(profile));
  const modal = session.modal;
  let showNerdStats = $state(false);

  function toggleNerdStats(): void {
    showNerdStats = !showNerdStats;
    session.setNerdStatsEnabled(showNerdStats);
  }

  setGameSessionContext(session);
</script>

<div
  class={`shell ${profile.mode === "mobile" ? "mobile-shell" : ""}${showNerdStats ? " nerd-stats-open" : ""}`}
  style={`--field-aspect-scale: ${profile.fieldAspectScale};`}
>
  {#if profile.ui.portraitOnly}
    <div class="orientation-blocker">
      <strong>Rotate to portrait</strong>
      <span>Vector Defence mobile is tuned for upright play.</span>
    </div>
  {/if}
  <ChromeBar />
  <GameBoard />
  <TowerPanel />

  {#if profile.ui.showFootnote}
    <p class="footnote" inert={$modal !== null}>
      Tip: press the tower keys shown on available buttons, <strong>U</strong> to upgrade, <strong>Esc</strong> to cancel build mode, and <strong>Space</strong> to pause or resume. Drag the board to pan, scroll to zoom, <strong>Shift</strong>+scroll or <strong>↑</strong>/<strong>↓</strong> to tilt, and <strong>0</strong> to reset the view.
      <button class="footnote-link" type="button" onclick={toggleNerdStats}>
        {showNerdStats ? "Hide" : "Show"} stats for nerds
      </button>
    </p>
  {/if}
  {#if profile.ui.showFootnote && showNerdStats}
    <NerdStatsPanel />
  {/if}
</div>
