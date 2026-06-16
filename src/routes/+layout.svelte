<!--
  src/routes/+layout.svelte
  Root layout — Import global CSS, setup font, render slot
-->
<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';

  // Disable context menu trên toàn ứng dụng (không cần chuột phải khi vẽ)
  onMount(() => {
    document.addEventListener('contextmenu', (e) => e.preventDefault());
  });
</script>

<slot />
