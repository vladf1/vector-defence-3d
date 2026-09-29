<script lang="ts">
  import { onMount } from "svelte";
  import { getGameSessionContext } from "../game-context";
  import { RendererStatus, shouldShowStartupTimings } from "../renderer-status";

  const session = getGameSessionContext();
  const rendererStatus = session.rendererStatus;
  const startupTimings = session.startupTimings;
  const showTimings = shouldShowStartupTimings(window);
  let sceneCanvas: HTMLCanvasElement;
  let overlayCanvas: HTMLCanvasElement;

  onMount(() => {
    session.mount({ canvas: sceneCanvas, overlay: overlayCanvas });

    return () => {
      session.unmount();
    };
  });
</script>

<canvas bind:this={sceneCanvas} class="board-canvas board-background board-depth" aria-hidden="true"></canvas>
<canvas
  bind:this={overlayCanvas}
  class="board-canvas board-game board-depth-overlay"
  id="game"
  onpointermove={session.handleCanvasMove}
  onpointerleave={session.handleCanvasLeave}
  onpointerdown={session.handleCanvasDown}
  onpointerup={session.handleCanvasUp}
  onpointercancel={session.handleCanvasUp}
  oncontextmenu={session.handleCanvasContextMenu}
></canvas>
{#if showTimings && $startupTimings}
  <dl class="board-timings">
    {#each Object.entries($startupTimings) as [phase, value] (phase)}
      <dt>{phase}</dt>
      <dd>{Math.round(value)}</dd>
    {/each}
  </dl>
{/if}
{#if $rendererStatus === RendererStatus.Loading}
  <div class="board-loading" role="status" aria-live="polite">
    <span class="board-loading-spinner" aria-hidden="true"></span>
    <span>Building 3D battlefield</span>
  </div>
{:else if $rendererStatus === RendererStatus.Failed}
  <div class="board-loading board-unsupported" role="alert">
    <strong>WebGPU required</strong>
    <span>Vector Defence renders with WebGPU. Try a current Chrome, Edge, or Safari 26.</span>
  </div>
{/if}
