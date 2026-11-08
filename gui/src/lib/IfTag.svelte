<script lang="ts">
  // An `if`, marked by whether it holds in Author mode's profile.
  let { condition, applies }: { condition: string; applies: boolean | null | undefined } = $props();
</script>

<span
  class="iftag"
  class:yes={applies === true}
  class:no={applies === false}
  title={applies === true ? "Holds in this profile" : applies === false ? "Doesn't hold in this profile" : "Can't be read"}
  >if <code>{condition}</code></span
>

<style>
  .iftag {
    font-size: 11px;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
    color: var(--dim);
    white-space: nowrap;
  }
  .iftag code {
    font-size: 11px;
  }
  .iftag.yes {
    color: var(--good);
    border-color: var(--good);
  }
  .iftag.no {
    color: var(--faint);
    border-style: dashed;
  }
</style>
