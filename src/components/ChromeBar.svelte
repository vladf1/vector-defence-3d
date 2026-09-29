<script lang="ts">
  import ControlIcon from "./ControlIcon.svelte";
  import { getGameSessionContext } from "../game-context";
  import { formatMoney } from "../utils";

  const session = getGameSessionContext();
  const { hud, modal, soundEnabled } = session;
  const profile = session.profile;

  function formatLevel(levelNumber: number | undefined): string {
    const value = levelNumber ?? "?";
    return profile.mode === "mobile" ? `Level ${value}` : String(value);
  }

  function formatProgress(current: number | undefined, total: number | undefined): string {
    return current === undefined || total === undefined ? "" : `${current} of ${total}`;
  }
</script>

<header class="topbar" inert={$modal !== null}>
  {#if profile.ui.showTitle}
    <div class="title-block">
      <h1>Vector Defence</h1>
    </div>
  {/if}
  {#if $hud.showStatusHud}
    <section class="hud">
      {#each [
        { label: profile.mode === "mobile" ? "" : "Level", value: formatLevel($hud.levelNumber), className: "level-stat" },
        { label: profile.mode === "mobile" ? "" : "Money", value: formatMoney($hud.money), className: "money-stat" },
        { label: profile.mode === "mobile" ? "" : "Wave", value: formatProgress($hud.waveCurrent, $hud.waveTotal), className: "wave-stat" },
        ...(profile.mode === "mobile" || $hud.waveMonsterTotal === undefined ? [] : [
          { label: "Monsters", value: formatProgress($hud.waveMonstersSpawned, $hud.waveMonsterTotal), className: "monsters-stat" },
        ]),
      ].filter((stat) => stat.value) as stat}
        <div class={`stat-card ${stat.className}`}>
          {#if stat.label}
            <span>{stat.label}</span>
          {/if}
          <strong>{stat.value}</strong>
        </div>
      {/each}
    </section>
  {/if}
  <div class="actions">
    <button
      class="chrome-button sound-button"
      type="button"
      aria-label={$soundEnabled ? "Mute sound" : "Unmute sound"}
      aria-pressed={$soundEnabled}
      title={$soundEnabled ? "Mute sound" : "Unmute sound"}
      onclick={session.toggleSound}
    >
      <ControlIcon kind={$soundEnabled ? "sound-on" : "sound-muted"} />
    </button>
    <button
      class="chrome-button pause-button"
      type="button"
      aria-label={$hud.paused ? "Resume" : "Pause"}
      title={$hud.paused ? "Resume" : "Pause"}
      disabled={!$hud.canTogglePause}
      onclick={session.togglePause}
    >
      <ControlIcon kind={$hud.paused ? "play" : "pause"} />
    </button>
    <button
      class="chrome-button campaign-button"
      type="button"
      aria-label="Campaign"
      title="Campaign"
      onclick={session.openMenu}
    >
      <ControlIcon kind="home" />
    </button>
  </div>
</header>
