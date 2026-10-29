<script lang="ts">
  import ExerciseList from "./lib/ExerciseList.svelte";
  import ExercisePage from "./lib/ExercisePage.svelte";

  let path = $state(window.location.pathname);

  const exerciseId = $derived.by(() => {
    const match = path.match(/^\/exercises\/([^/]+)\/?$/);
    return match ? decodeURIComponent(match[1]) : null;
  });

  function navigate(to: string) {
    if (to === window.location.pathname) return;
    history.pushState({}, "", to);
    path = to;
    window.scrollTo(0, 0);
  }

  // Turn clicks on same-site links into in-app navigation.
  function onLinkClick(event: MouseEvent) {
    if (event.defaultPrevented || event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey)
      return;

    const link = (event.target as Element | null)?.closest("a");
    if (!link || link.target || link.hasAttribute("download")) return;
    if (link.origin !== window.location.origin) return;

    event.preventDefault();
    navigate(link.pathname);
  }
</script>

<svelte:window
  onpopstate={() => (path = window.location.pathname)}
  onclick={onLinkClick}
/>

<svelte:head>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link
    rel="preconnect"
    href="https://fonts.gstatic.com"
    crossorigin="anonymous"
  />
  <link
    rel="stylesheet"
    href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500&family=Schibsted+Grotesk:wght@400;500;700&display=swap"
  />
</svelte:head>

<div class="app-shell">
  <header class="topbar">
    <div class="wrap bar">
      <a class="brand" href="/">
        <span class="brand-icon" aria-hidden="true">&gt;_</span>
        <span>PowerShell Lab</span>
      </a>
    </div>
  </header>

  <div class="page">
    {#if exerciseId}
      {#key exerciseId}
        <ExercisePage id={exerciseId} />
      {/key}
    {:else if path === "/"}
      <ExerciseList />
    {:else}
      <main class="not-found wrap">
        <h1>Page not found</h1>
        <p><a href="/">Back to all exercises</a></p>
      </main>
    {/if}
  </div>

  <footer class="footer">
    <div class="wrap foot">
      <span>Learn by doing. One script at a time.</span>
      <span>Local development build</span>
    </div>
  </footer>
</div>

<style>
  :global(:root) {
    color-scheme: light;
    --ink: #16212e;
    --muted: #55657a;
    --page: #eef1f6;
    --line: #d9e0ea;
    --console: #012456;
    --console-deep: #011a3f;
    --console-floor: #011230;
    --console-line: #1d4286;
    --console-text: #eef3ff;
    --console-dim: #9db4dc;
    --signal: #ffd84d;
    --ok: #7be0a0;
    --err: #ff9a9a;
    --sans: "Schibsted Grotesk", "Segoe UI", system-ui, sans-serif;
    --mono: "JetBrains Mono", "Cascadia Code", Consolas, monospace;
  }

  :global(#app) {
    width: 100%;
    max-width: none;
    margin: 0;
    padding: 0;
    text-align: left;
  }

  :global(*) {
    box-sizing: border-box;
  }

  :global(body) {
    display: block;
    min-width: 0;
    margin: 0;
    background: var(--page);
    color: var(--ink);
    font-family: var(--sans);
    -webkit-font-smoothing: antialiased;
  }

  :global(button) {
    font: inherit;
  }

  :global(.wrap) {
    width: 100%;
    max-width: 1240px;
    margin-inline: auto;
    padding-inline: clamp(20px, 4vw, 40px);
  }

  .app-shell {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
  }

  .page {
    flex: 1;
  }

  .not-found {
    padding-block: 56px;
  }

  .topbar {
    background: #fff;
    border-bottom: 1px solid var(--line);
  }

  .bar {
    display: flex;
    align-items: center;
    height: 62px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    color: inherit;
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.01em;
    text-decoration: none;
  }

  .brand-icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: var(--console);
    color: var(--signal);
    font-family: var(--mono);
    font-size: 13px;
    font-weight: 500;
  }

  .footer {
    border-top: 1px solid var(--line);
    color: var(--muted);
    font-size: 13px;
  }

  .foot {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding-block: 20px;
  }

  @media (max-width: 900px) {
    .foot {
      flex-direction: column;
      gap: 4px;
    }
  }
</style>
