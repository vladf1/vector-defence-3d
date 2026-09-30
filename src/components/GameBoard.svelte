<script lang="ts">
  import { onMount } from "svelte";
  import { getGameSessionContext } from "../game-context";
  import BoardSurface from "./BoardSurface.svelte";

  const session = getGameSessionContext();
  const hud = session.hud;
  const modal = session.modal;
  const helpOpen = session.helpOpen;

  onMount(() => () => {
    session.destroy();
  });

  function handleSkipBreak(event: MouseEvent): void {
    event.stopPropagation();
    session.skipBreak();
  }
</script>

<svelte:window onkeydown={session.handleKeyDown} />

<section class="board" inert={$modal !== null || $helpOpen}>
  <BoardSurface />
  {#if $hud.banner}
    {#key $hud.banner.startsWith("NEXT WAVE") ? "countdown" : $hud.banner}
      {#if $hud.canSkipBreak}
        <button type="button" class="board-banner skippable" aria-label="Start the next wave now" onclick={handleSkipBreak}>
          <span class="board-banner-text">{$hud.banner}</span>
          <span class="board-banner-action" aria-hidden="true">Skip ›</span>
        </button>
      {:else}
        <div class="board-banner" role="status" aria-live="polite">
          <span class="board-banner-text">{$hud.banner}</span>
        </div>
      {/if}
    {/key}
  {/if}
</section>
