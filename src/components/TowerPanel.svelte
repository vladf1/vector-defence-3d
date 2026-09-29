<script lang="ts">
  import { TOWER_CLASSES } from "../entities/towers/tower-registry";
  import { getGameSessionContext } from "../game-context";
  import type { TowerKind } from "../types";
  import { formatMoney } from "../utils";
  import { TOWER_ICON_SVG } from "./tower-icons";

  const session = getGameSessionContext();
  const profile = session.profile;
  const hud = session.hud;
  const modal = session.modal;

  function formatShortcuts(shortcuts: readonly string[]): string {
    return shortcuts.map((shortcut) => shortcut.toUpperCase()).join("/");
  }

  function handleTowerButtonClick(event: MouseEvent & { currentTarget: EventTarget & HTMLButtonElement }): void {
    session.toggleTowerPlacement(event.currentTarget.value as TowerKind);
  }

  function handleTowerButtonPointerDown(
    event: PointerEvent & { currentTarget: EventTarget & HTMLButtonElement },
  ): void {
    session.handleTowerButtonPointerDown(event.currentTarget.value as TowerKind, event);
  }
</script>

<section class:selected-tower-controls={profile.mode === "mobile" && $hud.hasSelectedTower} class="controls-grid" inert={$modal !== null}>
  <div class="tower-strip-card">
    <div class="tower-strip">
      {#each TOWER_CLASSES as towerClass (towerClass.kind)}
        {#if $hud.availableTowers.includes(towerClass.kind)}
          {@const shortcutText = formatShortcuts(towerClass.shortcuts)}
          {@const canAffordTower = $hud.affordableTowers[towerClass.kind]}
          <button
            class={`tower-button${$hud.placingTower === towerClass.kind ? " active" : ""}${canAffordTower ? "" : " unaffordable"}`}
            type="button"
            value={towerClass.kind}
            title={`${towerClass.label} tower: ${canAffordTower ? towerClass.summary : `need ${formatMoney(towerClass.baseCost)}`}`}
            aria-label={`${towerClass.label} tower for ${formatMoney(towerClass.baseCost)}. ${canAffordTower ? towerClass.summary : "Not enough money yet."} Shortcuts ${shortcutText}.`}
            disabled={$hud.towerButtonsDisabled}
            onclick={handleTowerButtonClick}
            onpointerdown={handleTowerButtonPointerDown}
          >
            <div class="tower-button-meta">
              <span>{formatMoney(towerClass.baseCost)}</span>
              {#if profile.ui.showShortcutLabels}
                <span class="shortcut-chip">{shortcutText}</span>
              {/if}
            </div>
            <span class="tower-icon" aria-hidden="true">{@html TOWER_ICON_SVG[towerClass.kind]}</span>
          </button>
        {/if}
      {/each}
    </div>
  </div>

  <div
    class:disabled-selection-panel={$hud.towerButtonsDisabled}
    class:tower-border-panel={$hud.towerButtonsDisabled || (profile.mode !== "mobile" && !$hud.hasSelectedTower && !$hud.placingTower)}
    class:idle-selection-card={profile.mode === "mobile" && !$hud.hasSelectedTower && !$hud.placingTower}
    class="control-card selection-card"
    aria-disabled={$hud.towerButtonsDisabled}
  >
    <div class="selection-header">
      <div class="selection-copy">
        {#if profile.mode === "mobile" && !$hud.hasSelectedTower && !$hud.placingTower}
          <strong>Build towers</strong>
          <span>Tap a tower to inspect</span>
        {:else if $hud.selectionName}
          <strong>{$hud.selectionName}</strong>
        {/if}
        {#if !(profile.mode === "mobile" && !$hud.hasSelectedTower && !$hud.placingTower) && $hud.selectionSummary}
          <span>{$hud.selectionSummary}</span>
        {/if}
      </div>
      {#if $hud.placingTower && !$hud.hasSelectedTower}
        <button
          class="action-button cancel-build-button"
          type="button"
          aria-label="Cancel build"
          title="Cancel build"
          onclick={session.cancelBuild}
          disabled={$hud.cancelBuildDisabled}
        >
          <span aria-hidden="true">×</span>
        </button>
      {/if}
      {#if $hud.hasSelectedTower && profile.mode !== "mobile"}
        <button
          class="action-button sell selection-sell-button"
          type="button"
          aria-label="Sell"
          title="Sell"
          onclick={session.sellSelectedTower}
          disabled={$hud.sellDisabled}
        >
          <span aria-hidden="true">💰</span>
        </button>
      {/if}
    </div>
    {#if profile.mode === "mobile" && $hud.hasSelectedTower}
      <div
        class:laser-actions={$hud.hasLaserLockAction}
        class="mobile-selection-actions"
        role="group"
        aria-label="Selected tower actions"
      >
        <button
          class={`action-button${$hud.upgradeUnaffordable ? " unaffordable" : ""}`}
          type="button"
          aria-label={$hud.upgradeLabel}
          title={$hud.upgradeLabel}
          onclick={session.upgradeSelectedTower}
          disabled={$hud.upgradeDisabled}
        >
          <span aria-hidden="true">▲</span>
          <span class="mobile-action-value">{$hud.upgradeValue}</span>
        </button>
        {#if $hud.hasLaserLockAction}
          <button
            class="action-button"
            type="button"
            aria-label={$hud.laserLocked ? "Unlock" : "Lock"}
            title={$hud.laserLocked ? "Unlock" : "Lock"}
            onclick={session.toggleSelectedLaserLock}
            disabled={$hud.laserLockDisabled}
          >
            <span aria-hidden="true">{$hud.laserLocked ? "🔓" : "🔒"}</span>
            <span class="mobile-action-value">{$hud.laserLocked ? "Unlock" : "Lock"}</span>
          </button>
        {/if}
        <button
          class="action-button sell"
          type="button"
          aria-label={$hud.sellLabel}
          title={$hud.sellLabel}
          onclick={session.sellSelectedTower}
          disabled={$hud.sellDisabled}
        >
          <span aria-hidden="true">💰</span>
          <span class="mobile-action-value">{$hud.sellValue}</span>
        </button>
      </div>
    {/if}
  </div>
</section>
