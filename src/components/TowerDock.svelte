<script lang="ts">
  import { getGameSessionContext } from "../game-context";
  import type { TowerCatalogEntry, TowerKind } from "../types";
  import { formatMoney } from "../utils";
  import { TOWER_ACCENTS, TOWER_ICON_SVG } from "./tower-icons";

  let { element = $bindable() }: { element?: HTMLElement } = $props();

  const session = getGameSessionContext();
  const { hud, modal, towerCatalog } = session;
  const profile = session.profile;
  const mobile = profile.mode === "mobile";

  const catalogEntry = (kind: TowerKind | undefined): TowerCatalogEntry | undefined =>
    kind === undefined ? undefined : $towerCatalog.find((entry) => entry.kind === kind);

  const selected = $derived(catalogEntry($hud.selectedTowerKind));
  const placing = $derived($hud.hasSelectedTower ? undefined : catalogEntry($hud.placingTower));
  const available = $derived($towerCatalog.filter((entry) => $hud.availableTowers.includes(entry.kind)));
  const levelPips = $derived(Array.from({ length: $hud.selectedTowerMaxLevel ?? 0 }, (_, index) => index < ($hud.selectedTowerLevel ?? 0)));

  function handleTowerButtonClick(event: MouseEvent & { currentTarget: EventTarget & HTMLButtonElement }): void {
    session.toggleTowerPlacement(event.currentTarget.value as TowerKind);
  }

  function handleTowerButtonPointerDown(event: PointerEvent & { currentTarget: EventTarget & HTMLButtonElement }): void {
    session.handleTowerButtonPointerDown(event.currentTarget.value as TowerKind, event);
  }
</script>

{#snippet towerIcon(kind: TowerKind)}
  <span class="tower-icon" aria-hidden="true">{@html TOWER_ICON_SVG[kind]}</span>
{/snippet}

<footer class="dock-band" inert={$modal !== null} class:has-selection={$hud.hasSelectedTower}>
  <div class="dock panel" bind:this={element} aria-label="Build towers">
    {#each available as tower (tower.kind)}
      {@const affordable = $hud.affordableTowers[tower.kind]}
      <button
        class="tower-button"
        class:active={$hud.placingTower === tower.kind}
        class:unaffordable={!affordable}
        style={`--accent: ${TOWER_ACCENTS[tower.kind]}`}
        type="button"
        value={tower.kind}
        title={`${tower.label}: ${affordable ? tower.summary : `needs ${formatMoney(tower.baseCost)}`}`}
        aria-label={`${tower.label} tower, ${formatMoney(tower.baseCost)}. ${tower.summary} Shortcut ${tower.shortcuts[0].toUpperCase()}.`}
        disabled={$hud.towerButtonsDisabled}
        onclick={handleTowerButtonClick}
        onpointerdown={handleTowerButtonPointerDown}
      >
        {@render towerIcon(tower.kind)}
        <span class="tower-cost">{formatMoney(tower.baseCost)}</span>
        {#if profile.ui.showShortcutLabels}
          <kbd class="tower-key">{tower.shortcuts[0].toUpperCase()}</kbd>
        {/if}
      </button>
    {/each}
  </div>

  {#if selected}
    <section class="selection panel" style={`--accent: ${TOWER_ACCENTS[selected.kind]}`} aria-label="Selected tower">
      <div class="selection-info">
        {@render towerIcon(selected.kind)}
        <div class="selection-text">
          <strong>{selected.label}</strong>
          <span class="level-pips" aria-label={`Level ${$hud.selectedTowerLevel} of ${$hud.selectedTowerMaxLevel}`}>
            {#each levelPips as lit, index (index)}<i class:lit></i>{/each}
          </span>
          <small>Range {$hud.selectedTowerRange}</small>
        </div>
      </div>
      <div class="selection-actions">
        <button
          class="action-button upgrade"
          class:unaffordable={$hud.upgradeUnaffordable}
          type="button"
          aria-label={$hud.upgradeLabel}
          title={$hud.upgradeValue === "Max" ? "Fully upgraded" : `Upgrade for ${$hud.upgradeValue} (U)`}
          disabled={$hud.upgradeDisabled}
          onclick={session.upgradeSelectedTower}
        >
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M4 12L10 6L16 12M4 16L10 10L16 16" /></svg>
          <span>{$hud.upgradeValue}</span>
        </button>
        {#if $hud.hasLaserLockAction}
          <button
            class="action-button"
            class:on={$hud.laserLocked}
            type="button"
            aria-label={$hud.laserLocked ? "Unlock beam direction" : "Lock beam direction"}
            title={$hud.laserLocked ? "Unlock the beam" : "Lock the beam direction"}
            disabled={$hud.laserLockDisabled}
            onclick={session.toggleSelectedLaserLock}
          >
            <svg viewBox="0 0 20 20" aria-hidden="true">
              <rect x="4.5" y="9" width="11" height="8" rx="1.5" />
              <path d={$hud.laserLocked ? "M7 9V6.5A3 3 0 0 1 13 6.5V9" : "M7 9V6.5A3 3 0 0 1 12.6 5"} />
            </svg>
            <span>{$hud.laserLocked ? "Locked" : "Lock"}</span>
          </button>
        {/if}
        <button
          class="action-button sell"
          type="button"
          aria-label={$hud.sellLabel}
          title={`Sell for ${$hud.sellValue}`}
          disabled={$hud.sellDisabled}
          onclick={session.sellSelectedTower}
        >
          <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="6.5" /><path d="M12.2 7.6Q10 6.4 8.4 7.4T9.6 10.2T11.8 12.8Q10 13.8 7.6 12.4M10 5V15" /></svg>
          <span>{$hud.sellValue}</span>
        </button>
        <button class="action-button close" type="button" aria-label="Deselect tower" title="Deselect" onclick={session.deselectTower}>
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M6 6L14 14M14 6L6 14" /></svg>
        </button>
      </div>
    </section>
  {:else if placing}
    <section class="selection panel placing" style={`--accent: ${TOWER_ACCENTS[placing.kind]}`} aria-label="Placing tower">
      <div class="selection-info">
        {@render towerIcon(placing.kind)}
        <div class="selection-text">
          <strong>Place {placing.label}</strong>
          <small>{mobile ? `Tap the field · ${formatMoney(placing.baseCost)}` : placing.summary}</small>
        </div>
      </div>
      <div class="selection-actions">
        <button
          class="action-button close"
          type="button"
          aria-label="Cancel build"
          title="Cancel (Esc)"
          disabled={$hud.cancelBuildDisabled}
          onclick={session.cancelBuild}
        >
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M6 6L14 14M14 6L6 14" /></svg>
        </button>
      </div>
    </section>
  {/if}
</footer>
