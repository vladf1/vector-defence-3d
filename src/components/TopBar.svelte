<script lang="ts">
  import ControlIcon from "./ControlIcon.svelte";
  import { getGameSessionContext } from "../game-context";
  import { formatMoney } from "../utils";

  let { element = $bindable() }: { element?: HTMLElement } = $props();

  const session = getGameSessionContext();
  const { hud, modal, soundEnabled } = session;
  const mobile = session.profile.mode === "mobile";

  const pad = (value: number | undefined): string => (value === undefined ? "--" : String(value).padStart(2, "0"));
  const ratio = (value: number | undefined, total: number | undefined): number =>
    value === undefined || !total ? 0 : Math.max(0, Math.min(1, value / total));

  // Every wave is shown as a pip; once the last wave is cleared they all stay lit.
  const wavePips = $derived(
    Array.from({ length: $hud.waveTotal }, (_, index) => {
      if ($hud.waveCurrent === undefined || index + 1 < $hud.waveCurrent) {
        return "done";
      }
      return index + 1 === $hud.waveCurrent ? "live" : "idle";
    }),
  );
  const integrity = $derived(ratio($hud.escapesLeft, $hud.escapesAllowed));
</script>

<header class="topbar" bind:this={element} inert={$modal !== null}>
  <div class="brand panel">
    <svg class="brand-mark" viewBox="0 0 40 40" aria-hidden="true">
      <path d="M20 3L36 12V28L20 37L4 28V12Z" />
      <path d="M20 11L28.5 26H11.5Z" />
      <circle cx="20" cy="21.5" r="2.2" />
    </svg>
    <div class="brand-text">
      <h1 class="brand-name">Vector Defence</h1>
      {#if $hud.showStatusHud}
        <span class="brand-level">
          <b>{mobile ? `L${$hud.levelNumber ?? "?"}` : `Level ${pad($hud.levelNumber)}`}</b>
          {#if !mobile && $hud.levelName}<span>{$hud.levelName}</span>{/if}
        </span>
      {/if}
    </div>
  </div>

  {#if $hud.showStatusHud}
    <section class="stats panel" aria-label="Battle status">
      {#if mobile}
        <div class="stat stat-level">
          <span class="stat-label">Level</span>
          <strong class="stat-value">{$hud.levelNumber ?? "?"}</strong>
        </div>
      {/if}
      <div class="stat stat-money">
        <span class="stat-label">Credits</span>
        <strong class="stat-value">{formatMoney($hud.money)}</strong>
      </div>
      <div class="stat stat-wave">
        <span class="stat-label">Wave</span>
        <strong class="stat-value">{$hud.waveCurrent ?? $hud.waveTotal}<small>/{$hud.waveTotal}</small></strong>
        <span class="pips" aria-hidden="true">
          {#each wavePips as pip, index (index)}<i class={pip}></i>{/each}
        </span>
      </div>
      {#if !mobile}
        <div class="stat stat-hostiles">
          <span class="stat-label">Hostiles</span>
          <strong class="stat-value">
            {$hud.waveMonstersSpawned ?? 0}<small>/{$hud.waveMonsterTotal ?? 0}</small>
          </strong>
          <span class="meter" aria-hidden="true">
            <i style={`--fill: ${ratio($hud.waveMonstersSpawned, $hud.waveMonsterTotal)}`}></i>
          </span>
        </div>
      {/if}
      <div class="stat stat-integrity" class:critical={integrity <= 0.25}>
        <span class="stat-label">{mobile ? "Lives" : "Breach limit"}</span>
        <strong class="stat-value">{$hud.escapesLeft ?? 0}</strong>
        <span class="meter" aria-hidden="true"><i style={`--fill: ${integrity}`}></i></span>
      </div>
    </section>
  {/if}

  <nav class="actions panel" aria-label="Game controls">
    <button
      class="icon-button help-button"
      type="button"
      aria-label="How to play"
      title="How to play (?)"
      onclick={() => session.setHelpOpen(true)}
    >
      <ControlIcon kind="help" />
    </button>
    <button
      class="icon-button sound-button"
      type="button"
      aria-label={$soundEnabled ? "Mute sound" : "Unmute sound"}
      aria-pressed={$soundEnabled}
      title={$soundEnabled ? "Mute sound" : "Unmute sound"}
      onclick={session.toggleSound}
    >
      <ControlIcon kind={$soundEnabled ? "sound-on" : "sound-muted"} />
    </button>
    <button
      class="icon-button pause-button"
      type="button"
      aria-label={$hud.paused ? "Resume" : "Pause"}
      title={$hud.paused ? "Resume (Space)" : "Pause (Space)"}
      disabled={!$hud.canTogglePause}
      onclick={session.togglePause}
    >
      <ControlIcon kind={$hud.paused ? "play" : "pause"} />
    </button>
    <button class="icon-button campaign-button" type="button" aria-label="Campaign" title="Campaign map" onclick={session.openMenu}>
      <ControlIcon kind="home" />
    </button>
  </nav>
</header>
