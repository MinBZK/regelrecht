<script lang="ts">
  import { Handle, Position, type NodeProps } from '@xyflow/svelte';

  type $$Props = NodeProps & { data: { label: string; tooltip?: string } };

  let { data, sourcePosition, targetPosition }: $$Props = $props();
</script>

<div class="leaf-node" title={data.tooltip || ''}>
  {data.label}
</div>

{#if targetPosition}
  <Handle type="target" position={targetPosition} />
{/if}
{#if sourcePosition}
  <Handle type="source" position={sourcePosition} />
{/if}

<style>
  .leaf-node {
    padding: 4px 8px;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: default;
  }
</style>
