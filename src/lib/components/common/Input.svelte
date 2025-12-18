<script lang="ts">
  type Props = {
    value?: string;
    placeholder?: string;
    type?: 'text' | 'search' | 'number';
    disabled?: boolean;
    class?: string;
    oninput?: (e: Event) => void;
    onfocus?: (e: FocusEvent) => void;
    onblur?: (e: FocusEvent) => void;
    onkeydown?: (e: KeyboardEvent) => void;
  }

  let {
    value = $bindable(''),
    placeholder = '',
    type = 'text',
    disabled = false,
    class: className = '',
    oninput,
    onfocus,
    onblur,
    onkeydown
  }: Props = $props();
</script>

<input
  {type}
  {placeholder}
  {disabled}
  bind:value
  class="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-md text-zinc-100 placeholder-zinc-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:opacity-50 disabled:cursor-not-allowed {className}"
  oninput={oninput}
  onfocus={onfocus}
  onblur={onblur}
  onkeydown={onkeydown}
/>
