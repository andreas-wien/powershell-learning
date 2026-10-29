<script lang="ts">
  import { onMount } from "svelte";

  type ExerciseSummary = {
    id: string;
    title: string;
    description?: string;
  };

  let exercises: ExerciseSummary[] = [];
  let loading = true;
  let error = "";

  onMount(async () => {
    try {
      const response = await fetch("/api/exercises");

      if (!response.ok) {
        throw new Error(`Could not load exercises (${response.status})`);
      }

      exercises = await response.json();
    } catch (err) {
      error = err instanceof Error ? err.message : "Could not load exercises.";
    } finally {
      loading = false;
    }
  });
</script>

<svelte:head>
  <title>PowerShell Lab</title>
  <meta
    name="description"
    content="Learn PowerShell by solving practical IT support exercises."
  />
</svelte:head>

<main class="home wrap">
  <h1>Exercises</h1>
  <p class="intro">
    Learn PowerShell by solving practical IT support exercises.
  </p>

  {#if loading}
    <p class="message">Loading exercises…</p>
  {:else if error}
    <p class="message error" role="alert">{error}</p>
  {:else if exercises.length === 0}
    <p class="message">No exercises available yet.</p>
  {:else}
    <ol class="list">
      {#each exercises as item, i (item.id)}
        <li>
          <a class="card" href={`/exercises/${encodeURIComponent(item.id)}`}>
            <span class="index">{String(i + 1).padStart(2, "0")}</span>
            <span class="text">
              <span class="title">{item.title}</span>
              {#if item.description}
                <span class="desc">{item.description}</span>
              {/if}
            </span>
            <span class="go" aria-hidden="true">→</span>
          </a>
        </li>
      {/each}
    </ol>
  {/if}
</main>

<style>
  .home {
    padding-block: 56px 72px;
  }

  h1 {
    color: #08060d;
    margin: 0 0 14px;
    font-size: clamp(30px, 3.4vw, 42px);
    font-weight: 700;
    line-height: 1.1;
    letter-spacing: -0.025em;
  }

  .intro {
    max-width: 46ch;
    margin: 0 0 36px;
    color: var(--muted);
    font-size: 17px;
    line-height: 1.6;
  }

  .message {
    margin: 0;
    color: var(--muted);
    font-size: 16px;
  }

  .error {
    color: #b42318;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 720px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .card {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 18px 20px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: #fff;
    color: inherit;
    text-decoration: none;
    transition:
      border-color 0.15s,
      box-shadow 0.15s;
  }

  .card:hover {
    border-color: var(--console);
    box-shadow: 0 10px 24px -14px rgb(1 36 86 / 45%);
  }

  .card:focus-visible {
    outline: 2px solid var(--console);
    outline-offset: 2px;
  }

  .index {
    flex: 0 0 auto;
    color: var(--console);
    font-family: var(--mono);
    font-size: 14px;
    font-weight: 500;
  }

  .text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .title {
    font-size: 17px;
    font-weight: 700;
  }

  .desc {
    color: var(--muted);
    font-size: 15px;
    line-height: 1.5;
  }

  .go {
    color: var(--console);
    font-size: 18px;
  }

  @media (prefers-reduced-motion: reduce) {
    .card {
      transition: none;
    }
  }
</style>
