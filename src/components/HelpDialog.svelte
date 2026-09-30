<script lang="ts">
  import ControlIcon from "./ControlIcon.svelte";
  import { getGameSessionContext } from "../game-context";
  import { formatMoney } from "../utils";
  import { TOWER_ACCENTS, TOWER_ICON_SVG } from "./tower-icons";

  const session = getGameSessionContext();
  const { helpOpen, towerCatalog } = session;
  const mobile = session.profile.mode === "mobile";

  const controls: readonly (readonly [string, string])[] = mobile
    ? [
      ["Tap a tower, then the field", "Build (or drag a tower onto the field)"],
      ["Tap a tower on the field", "Select it to upgrade, lock, or sell"],
      ["Tap the countdown", "Start the next wave early"],
    ]
    : [
      ["1 – 5 or G Z R S D E", "Pick a tower to build, then click the field"],
      ["Click a tower", "Select it; its upgrade button floats beside it"],
      ["U", "Upgrade the selected tower"],
      ["Esc", "Cancel building (or close this)"],
      ["Space", "Pause or resume"],
      ["Drag · Wheel", "Pan · zoom the battlefield"],
      ["Shift + Wheel · ↑ ↓", "Tilt the camera"],
      ["= −  ·  0", "Zoom · reset the view"],
      ["N", "Stats for nerds: frame rate, update and draw times, object counts"],
      ["?", "This help"],
    ];

  function focusClose(element: HTMLElement) {
    const opener = document.activeElement;
    queueMicrotask(() => element.querySelector<HTMLButtonElement>(".help-close")?.focus());
    return {
      destroy: () => queueMicrotask(() => {
        if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
      }),
    };
  }

  /** Keeps Tab and Shift+Tab inside the dialog (everything behind it is inert as well). */
  function containTab(event: KeyboardEvent & { currentTarget: HTMLElement }): void {
    if (event.key !== "Tab") return;
    const focusable = event.currentTarget.querySelectorAll<HTMLElement>("button:not(:disabled), [tabindex]:not([tabindex='-1'])");
    const first = focusable[0], last = focusable[focusable.length - 1];
    if (!first || !last) return;
    if (event.shiftKey && (document.activeElement === first || !event.currentTarget.contains(document.activeElement))) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function closeOnBackdrop(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      session.setHelpOpen(false);
    }
  }
</script>

{#if $helpOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="overlay help-overlay" onclick={closeOnBackdrop} use:focusClose>
    <div class="help panel" role="dialog" aria-modal="true" aria-labelledby="help-title" tabindex="-1" onkeydown={containTab}>
      <header class="help-header">
        <div>
          <span class="eyebrow">Field manual</span>
          <h2 id="help-title">How to play</h2>
        </div>
        <button class="icon-button help-close" type="button" aria-label="Close help" onclick={() => session.setHelpOpen(false)}>
          <ControlIcon kind="close" />
        </button>
      </header>

      <div class="help-body">
        <section class="help-brief">
          <p>
            Monsters march down the road toward the exit portal. Build towers beside the road to stop
            them. The number on the portal is how many may slip through; when it hits zero, the base
            is breached and the level is lost.
          </p>
          <p>
            Kills pay bounties and every cleared wave pays a reward. Upgrade a tower (up to level 7) for
            more damage and range, or sell it for 75% of what you spent. Clear every wave without a leak
            for three stars.
          </p>
        </section>

        <section>
          <h3>Towers</h3>
          <ul class="help-towers">
            {#each $towerCatalog as tower (tower.kind)}
              <li style={`--accent: ${TOWER_ACCENTS[tower.kind]}`}>
                <span class="tower-icon" aria-hidden="true">{@html TOWER_ICON_SVG[tower.kind]}</span>
                <div>
                  <strong>{tower.label} <small>{formatMoney(tower.baseCost)}</small></strong>
                  <span>{tower.summary}</span>
                </div>
                {#if !mobile}<kbd>{tower.shortcuts.map((key) => key.toUpperCase()).join(" / ")}</kbd>{/if}
              </li>
            {/each}
          </ul>
          <p class="help-note">Each level offers five of the six towers; where two share a number key, only one of them is in play.</p>
        </section>

        <section>
          <h3>Controls</h3>
          <dl class="help-controls">
            {#each controls as [keys, action] (keys)}
              <dt>{keys}</dt>
              <dd>{action}</dd>
            {/each}
          </dl>
        </section>
      </div>
    </div>
  </div>
{/if}
