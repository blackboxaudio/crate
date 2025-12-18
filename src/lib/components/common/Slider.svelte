<script lang="ts">
  type Props = {
    value?: number;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    class?: string;
    oninput?: (e: Event) => void;
    onchange?: (e: Event) => void;
  }

  let {
    value = $bindable(0),
    min = 0,
    max = 100,
    step = 1,
    disabled = false,
    class: className = '',
    oninput,
    onchange
  }: Props = $props();

  let percentage = $derived(((value - min) / (max - min)) * 100);
</script>

<input
  type="range"
  {min}
  {max}
  {step}
  {disabled}
  bind:value
  class="w-full h-1.5 bg-zinc-700 rounded-full appearance-none cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed {className}"
  style="background: linear-gradient(to right, #3b82f6 0%, #3b82f6 {percentage}%, #3f3f46 {percentage}%, #3f3f46 100%)"
  oninput={oninput}
  onchange={onchange}
/>

<style>
  input[type='range']::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #ffffff;
    cursor: pointer;
    transition: transform 0.1s;
  }

  input[type='range']::-webkit-slider-thumb:hover {
    transform: scale(1.2);
  }

  input[type='range']::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #ffffff;
    cursor: pointer;
    border: none;
  }
</style>
