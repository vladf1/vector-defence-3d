<script lang="ts">
  import { getGameSessionContext } from "../game-context";
  import type { ModalView } from "../types";

  const session = getGameSessionContext();
  const { modal } = session;
  const STAR_VALUES = [1, 2, 3];

  function focusModal(element: HTMLDivElement, _view: ModalView) {
    const opener = document.activeElement;
    const focusFirst = () => {
      if (element.isConnected && !element.contains(document.activeElement)) {
        (element.querySelector<HTMLButtonElement>("button:not(:disabled)") ?? element).focus();
      }
    };
    queueMicrotask(focusFirst);
    return {
      update: () => queueMicrotask(focusFirst),
      destroy: () => queueMicrotask(() => {
        if (!$modal && opener instanceof HTMLElement && opener.isConnected) opener.focus();
      }),
    };
  }

  function containTab(event: KeyboardEvent & { currentTarget: HTMLDivElement }): void {
    if (event.key !== "Tab") return;
    const buttons = event.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
    const first = buttons[0], last = buttons[buttons.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  }
</script>

{#snippet modalActions(view: ModalView)}
  {#if view.actions.length > 0}
    <div class="selection-actions">
      {#each view.actions as item}
        <button class={`modal-button modal-action-${item.action}`} type="button" onclick={() => session.handleModalAction(item.action)}>
          {item.label}
        </button>
      {/each}
    </div>
  {/if}
{/snippet}

{#if $modal}
  <div
    class={`modal${$modal.sheet ? " sheet" : ""}`}
    role="dialog"
    aria-modal="true"
    aria-label={$modal.title}
    tabindex="-1"
    use:focusModal={$modal}
    onkeydown={containTab}
  >
    <div class={`modal-panel${$modal.sheet ? " modal-sheet" : ""}${$modal.levelCards ? " level-map-panel" : ""}`}>
      {#if !$modal.levelCards}
        <h2>{$modal.title}</h2>
        <p>{$modal.description}</p>

        {#if $modal.starAward}
          <div
            class:perfect-award={$modal.starAward.perfect}
            class="star-award"
            aria-label={$modal.starAward.label}
          >
            <div class="star-award-row" aria-hidden="true">
              {#each STAR_VALUES as star}
                <span class:earned={star <= $modal.starAward.stars}>★</span>
              {/each}
            </div>
            <strong>{$modal.starAward.title}</strong>
            <span>{$modal.starAward.description}</span>
          </div>
        {/if}

        {@render modalActions($modal)}
      {:else}
        <div class="level-map-header">
          <div>
            <h2>{$modal.title}</h2>
            <p>{$modal.description}</p>
          </div>

          {@render modalActions($modal)}
        </div>

        <div class="level-grid">
          {#each $modal.levelCards as item}
            <button
              class={`level-card${item.unlocked ? "" : " locked"}${item.cleared ? " cleared" : ""}${item.current ? " current" : ""}`}
              type="button"
              disabled={!item.unlocked}
              onclick={() => session.selectLevel(item.index)}
            >
              <div class="level-card-heading">
                <strong>{item.title}</strong>
                <span class="level-pill">{item.status}</span>
              </div>
              {#if item.stars > 0}
                <span class="level-stars" aria-label={item.starsLabel}>
                  {#each STAR_VALUES as star}
                    <span class:earned={star <= item.stars} aria-hidden="true">★</span>
                  {/each}
                </span>
              {/if}
              <span>{item.description}</span>
              <small>{item.summary}</small>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/if}
