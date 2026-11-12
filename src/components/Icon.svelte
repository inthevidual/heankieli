<script lang="ts">
  const paths: Record<string, string> = {
    plus: "M12 5v14M5 12h14",
    play: "M7 4.5v15l12.5-7.5z",
    pause: "M7 4h3.5v16H7zM13.5 4H17v16h-3.5z",
    prev: "M6 5v14M19 5 9 12l10 7z",
    next: "M18 5v14M5 5l10 7-10 7z",
    back: "M11 18 5 12l6-6M19 18l-6-6 6-6",
    fwd: "M13 18l6-6-6-6M5 18l6-6-6-6",
    x: "M6 6l12 12M18 6 6 18",
    trash: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13",
    split: "M12 3v18M5 8l-3 4 3 4M19 8l3 4-3 4",
    merge: "M4 6h6v12H4zM14 6h6v12h-6z",
    insert: "M12 5v14M5 12h14",
    search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
    undo: "M9 14 4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3",
    redo: "M15 14l5-5-5-5M20 9H10a6 6 0 0 0 0 12h3",
    download: "M12 4v11M7 10l5 5 5-5M5 20h14",
    folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    cpu: "M7 7h10v10H7zM9 3v4M15 3v4M9 17v4M15 17v4M3 9h4M3 15h4M17 9h4M17 15h4",
    gear: "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
    check: "M5 12.5l4.5 4.5L19 7.5",
    alert: "M12 8v5M12 16.5v.5M10.3 3.9 2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
    film: "M4 4h16v16H4zM4 9h16M4 15h16M9 4v16M15 4v16",
    wave: "M3 12h2M7 8v8M11 5v14M15 9v6M19 7v10M21 12h0",
    copy: "M9 9h11v11H9zM5 15H4V4h11v1",
    refresh: "M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7",
    unlock: "M7 11V8a5 5 0 0 1 9.6-2M5 11h14v10H5z",
    target: "M12 3v4M12 17v4M3 12h4M17 12h4M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
    start: "M5 4v16M9 12h10M15 8l4 4-4 4",
    end: "M19 4v16M15 12H5M9 8l-4 4 4 4",
  };
  let { name, size = 17 }: { name: string; size?: number } = $props();
</script>

<svg viewBox="0 0 24 24" width={size} height={size} fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  {#if name === "play" || name === "pause"}
    <path d={paths[name]} fill="currentColor" stroke="none" />
  {:else}
    <path d={paths[name] ?? ""} />
  {/if}
</svg>
