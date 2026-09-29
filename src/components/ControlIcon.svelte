<script lang="ts">
  import { CONTROL_ICONS, type ControlIconKind } from "./control-icons";

  const { kind }: { kind: ControlIconKind } = $props();
</script>

<svg class="control-icon" viewBox="0 0 256 256" aria-hidden="true">
  {#each CONTROL_ICONS[kind] as stroke, strokeIndex (strokeIndex)}
    {#each stroke.layers as layer, layerIndex (layerIndex)}
      <path
        d={stroke.d}
        fill="none"
        stroke={layer.color}
        stroke-width={layer.width}
        stroke-opacity={layer.opacity}
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    {/each}
  {/each}
</svg>
