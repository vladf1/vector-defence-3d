<script lang="ts">
  import { getGameSessionContext } from "../game-context";

  const session = getGameSessionContext();
  const { hud } = session;
</script>

<section class="nerd-panel panel" aria-label="Stats for nerds">
  <header>
    <span>Stats for nerds</span>
    <kbd>N</kbd>
  </header>
  <dl>
    {#each [
      { label: "FPS", value: $hud.nerdStats.fps },
      { label: "Frame", value: $hud.nerdStats.frameTime },
      { label: "Update", value: $hud.nerdStats.updateTime },
      { label: "Draw", value: $hud.nerdStats.drawTime },
      { label: "Tracked", value: $hud.nerdStats.trackedObjects },
      { label: "Towers", value: $hud.nerdStats.towers },
      { label: "Hostiles", value: $hud.nerdStats.hostiles },
      { label: "Shots", value: $hud.nerdStats.shots },
      { label: "FX", value: $hud.nerdStats.effects },
    ] as stat (stat.label)}
      <dt>{stat.label}</dt>
      <dd>{stat.value}</dd>
    {/each}
  </dl>
</section>
