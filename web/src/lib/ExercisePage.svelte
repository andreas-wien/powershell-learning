<script lang="ts">
  import { onMount } from "svelte";
  import * as monaco from "monaco-editor";

  type Exercise = {
    id: string;
    title: string;
    description: string;
    instructions: string;
    starter_code?: string;
  };

  type ExerciseResult = {
    exercise_id: string;
    passed: boolean;
    message: string;
    stdout: string;
    stderr: string;
    exit_code: number | null;
    timed_out: boolean;
  };

  export let id: string;

  let editorContainer: HTMLDivElement;
  let editor: monaco.editor.IStandaloneCodeEditor | undefined;

  let exercise: Exercise | null = null;
  let result: ExerciseResult | null = null;
  let loading = true;
  let running = false;
  let error = "";

  async function loadExercise() {
    try {
      const response = await fetch(`/api/exercises/${encodeURIComponent(id)}`);

      if (!response.ok) {
        throw new Error(
          response.status === 404
            ? "Exercise not found."
            : `Could not load exercise (${response.status})`,
        );
      }

      const loaded: Exercise = await response.json();
      exercise = loaded;
      editor?.setValue(loaded.starter_code ?? "# Write your solution here\n");
    } catch (err) {
      error = err instanceof Error ? err.message : "Could not load exercise.";
    } finally {
      loading = false;
    }
  }

  async function runScript() {
    if (!editor || !exercise || running) return;

    running = true;
    result = null;
    error = "";

    try {
      const body = JSON.stringify({ script: editor.getValue() });

      // The backend rejects request bodies over 20 KB.
      if (new TextEncoder().encode(body).length > 20 * 1024) {
        throw new Error("Your script is too large to submit (20 KB maximum).");
      }

      const response = await fetch(
        `/api/exercises/${encodeURIComponent(exercise.id)}/submit`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body,
        },
      );

      if (!response.ok) {
        const message = await response.text();
        throw new Error(message || `Request failed (${response.status})`);
      }

      result = await response.json();
    } catch (err) {
      error = err instanceof Error ? err.message : "Script submission failed.";
    } finally {
      running = false;
    }
  }

  onMount(() => {
    // Start fetching first, so a Monaco setup problem can't block the API call.
    void loadExercise();

    // Classic PowerShell console blue, so the editor looks like the tool it teaches.
    monaco.editor.defineTheme("ps-console", {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "comment", foreground: "7f9cc9", fontStyle: "italic" },
        { token: "keyword", foreground: "8fd1ff" },
        { token: "string", foreground: "ffe9a3" },
        { token: "variable", foreground: "ffb86b" },
        { token: "number", foreground: "b5f0c4" },
      ],
      colors: {
        "editor.background": "#012456",
        "editor.lineHighlightBackground": "#0a2f6a",
        "editorLineNumber.foreground": "#6485bb",
        "editorLineNumber.activeForeground": "#cfe0ff",
        "editorCursor.foreground": "#ffd84d",
        "editor.selectionBackground": "#2a5cb8",
        "editorGutter.background": "#012456",
      },
    });

    editor = monaco.editor.create(editorContainer, {
      value: "# Write your solution here\n",
      language: "powershell",
      theme: "ps-console",
      automaticLayout: true,
      minimap: { enabled: false },
      fontFamily: '"JetBrains Mono", "Cascadia Code", Consolas, monospace',
      fontSize: 14,
      lineHeight: 22,
      lineNumbers: "on",
      lineNumbersMinChars: 3,
      lineDecorationsWidth: 12,
      glyphMargin: false,
      folding: false,
      scrollBeyondLastLine: false,
      renderLineHighlight: "line",
      fixedOverflowWidgets: true,
      tabSize: 4,
      padding: { top: 16, bottom: 16 },
    });

    // Web fonts load after Monaco measures its glyphs; re-measure to keep the cursor aligned.
    document.fonts.ready.then(() => monaco.editor.remeasureFonts());

    return () => editor?.dispose();
  });
</script>

<svelte:head>
  <title>{exercise?.title ?? "Exercise"} | PowerShell Lab</title>
</svelte:head>

<main class="workspace wrap">
  <section class="lesson">
    <a class="back" href="/">← All exercises</a>
    <h1>{exercise?.title ?? "Exercise"}</h1>

    {#if loading}
      <p class="description">Loading exercise…</p>
    {:else if exercise}
      <p class="description">{exercise.description}</p>

      <div class="task">
        <h2>Your task</h2>
        <p>{exercise.instructions}</p>
      </div>

      <p class="tip">
        <strong>New to PowerShell?</strong>
        <code>Write-Output</code> displays text. Put the text you want to display
        inside quotation marks.
      </p>
    {:else}
      <p class="description">This exercise couldn’t be loaded.</p>
    {/if}
  </section>

  <section class="console" aria-label="Script editor and output">
    <div class="console-bar">
      <span class="file">solution.ps1</span>
      <span>PowerShell</span>
    </div>

    <div class="editor-container" bind:this={editorContainer}></div>

    <div class="actions">
      <span class="run-note">Runs in the backend sandbox.</span>
      <button
        class="run-button"
        onclick={runScript}
        disabled={running || loading || !exercise}
        aria-busy={running}
      >
        {running ? "Running…" : "▶ Run script"}
      </button>
    </div>

    <div class="output" aria-live="polite">
      <div class="output-heading">
        <h3>Output</h3>
        {#if result}
          <span
            class="status"
            class:passed={result.passed}
            class:failed={!result.passed}
          >
            {result.passed ? "Passed" : "Not yet"}
          </span>
        {/if}
      </div>

      {#if error}
        <p class="error" role="alert">{error}</p>
      {:else if running}
        <p class="empty-output">Executing your script…</p>
      {:else if result}
        <p class="result-message" class:success-text={result.passed}>
          {result.message}
        </p>

        {#if result.stdout}
          <div class="output-label">Output</div>
          <pre>{result.stdout}</pre>
        {/if}

        {#if result.stderr}
          <div class="output-label">Errors</div>
          <pre class="stderr">{result.stderr}</pre>
        {/if}

        {#if result.timed_out}
          <p class="stderr">Execution timed out.</p>
        {/if}

        {#if !result.stdout && !result.stderr && !result.timed_out}
          <p class="empty-output">No output was produced.</p>
        {/if}
      {:else}
        <p class="empty-output">
          Run your script to see its output and check your answer.
        </p>
      {/if}
    </div>
  </section>
</main>

<style>
  /* Layout */
  .workspace {
    display: grid;
    flex: 1;
    grid-template-columns: minmax(260px, 360px) minmax(0, 1fr);
    gap: clamp(28px, 5vw, 64px);
    align-items: start;
    padding-block: 44px 56px;
  }

  /* Lesson column */
  .back {
    display: inline-block;
    margin-bottom: 20px;
    color: var(--muted);
    font-size: 14px;
    text-decoration: none;
  }

  .back:hover {
    color: var(--ink);
  }

  h1 {
    color: #08060d;
    margin: 0 0 14px;
    font-size: clamp(30px, 3.4vw, 42px);
    font-weight: 700;
    line-height: 1.1;
    letter-spacing: -0.025em;
  }

  .description {
    max-width: 46ch;
    margin: 0 0 32px;
    color: var(--muted);
    font-size: 17px;
    line-height: 1.6;
  }

  .task {
    padding-left: 18px;
    border-left: 3px solid var(--console);
  }

  .task h2 {
    margin: 0 0 6px;
    font-size: 15px;
    font-weight: 700;
  }

  .task p {
    margin: 0;
    font-size: 16px;
    line-height: 1.6;
    overflow-wrap: anywhere;
  }

  .tip {
    margin: 28px 0 0;
    padding: 14px 16px;
    border-radius: 8px;
    background: #e1e8f4;
    color: #2c3d56;
    font-size: 14px;
    line-height: 1.7;
  }

  .tip strong {
    display: block;
    color: var(--ink);
  }

  code {
    padding: 1px 6px;
    border-radius: 4px;
    background: #0d0a0f;
    color: var(--console-text);
    font-family: var(--mono);
    font-size: 13px;
  }

  /* Console: editor + output as one window */
  .console {
    min-width: 0;
    overflow: hidden;
    border-radius: 12px;
    background: var(--console);
    box-shadow: 0 18px 40px -18px rgb(1 36 86 / 55%);
  }

  .console-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 42px;
    padding: 0 18px;
    border-bottom: 1px solid var(--console-line);
    background: var(--console-deep);
    color: var(--console-dim);
    font-size: 13px;
  }

  .file {
    color: var(--console-text);
    font-family: var(--mono);
  }

  .editor-container {
    height: clamp(260px, 40vh, 420px);
    background: var(--console);
    color: var(--console-text);
    direction: ltr;
    text-align: left;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 18px;
    border-block: 1px solid var(--console-line);
    background: var(--console-deep);
  }

  .run-note {
    color: var(--console-dim);
    font-size: 13px;
  }

  .run-button {
    padding: 10px 20px;
    border: 0;
    border-radius: 8px;
    background: var(--signal);
    color: #0b1b3a;
    font-size: 14px;
    font-weight: 700;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .run-button:hover:enabled {
    background: #ffe37d;
  }

  .run-button:focus-visible {
    outline: 2px solid #fff;
    outline-offset: 2px;
  }

  .run-button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .run-button[aria-busy="true"] {
    cursor: wait;
  }

  /* Output */
  .output {
    min-height: 170px;
    padding: 18px;
    background: var(--console-floor);
  }

  .output-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 14px;
  }

  .output-heading h3 {
    margin: 0;
    color: var(--console-text);
    font-size: 15px;
    font-weight: 700;
  }

  .status {
    padding: 3px 10px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 700;
  }

  .passed {
    background: rgb(123 224 160 / 16%);
    color: var(--ok);
  }

  .failed {
    background: rgb(255 216 77 / 14%);
    color: var(--signal);
  }

  .result-message {
    margin: 0 0 14px;
    color: #ffe9a3;
    font-size: 14px;
    line-height: 1.55;
  }

  .success-text {
    color: var(--ok);
  }

  .output-label {
    margin: 16px 0 6px;
    color: var(--console-dim);
    font-size: 12px;
    font-weight: 500;
  }

  pre {
    margin: 0;
    padding: 12px 14px;
    overflow-x: auto;
    border-radius: 6px;
    background: #00091d;
    color: var(--console-text);
    font-family: var(--mono);
    font-size: 13px;
    line-height: 1.6;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .stderr {
    color: var(--err);
  }

  .empty-output {
    margin: 0;
    color: var(--console-dim);
    font-size: 14px;
    line-height: 1.6;
  }

  .error {
    margin: 0;
    color: var(--err);
    font-size: 14px;
    line-height: 1.55;
    overflow-wrap: anywhere;
  }

  @media (max-width: 900px) {
    .workspace {
      grid-template-columns: 1fr;
      gap: 28px;
      padding-top: 28px;
    }

    .editor-container {
      height: 260px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .run-button {
      transition: none;
    }
  }
</style>
