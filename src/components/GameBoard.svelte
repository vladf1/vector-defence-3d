<script lang="ts">
  import { onMount } from "svelte";
  import { getGameSessionContext } from "../game-context";
  import BoardSurface from "./BoardSurface.svelte";
  import GameModal from "./GameModal.svelte";

  const session = getGameSessionContext();
  const profile = session.profile;
  const hud = session.hud;
  const modal = session.modal;

  onMount(() => () => {
    session.destroy();
  });

  function handleSkipBreak(event: MouseEvent): void {
    event.stopPropagation();
    session.skipBreak();
  }
</script>

<svelte:window onkeydown={session.handleKeyDown} />

<section class="board-card">
  <div class="board-frame">
    <div
      class="board-stage"
      inert={$modal !== null}
      style={`--field-aspect-ratio: ${profile.fieldAspectRatio}; --field-aspect-scale: ${profile.fieldAspectScale};`}
    >
      <BoardSurface />
      {#if $hud.banner}
        {#if $hud.canSkipBreak}
          <button
            type="button"
            class="board-banner skippable"
            aria-label="Start the next wave now"
            onclick={handleSkipBreak}
          >
            <span class="board-banner-text">{$hud.banner}</span>
            <span class="board-banner-action" aria-hidden="true">SKIP »</span>
          </button>
        {:else}
          <div class="board-banner" role="status" aria-live="polite">
            <span class="board-banner-text">{$hud.banner}</span>
          </div>
        {/if}
      {/if}
    </div>
    <GameModal />
  </div>
</section>
