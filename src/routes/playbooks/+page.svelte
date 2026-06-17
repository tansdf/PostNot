<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { onMount } from "svelte";

  import {
    addPlaybookStep,
    cancelActiveRequest,
    createPlaybook,
    createPlaybookRun,
    deletePlaybook,
    deletePlaybookStep,
    duplicatePlaybook,
    finishPlaybookRun,
    getEnvironment,
    getPlaybook,
    getPlaybookExecutionContext,
    getPlaybookRun,
    listEnvironments,
    listPlaybookRuns,
    listPlaybooks,
    recordPlaybookRunStep,
    reorderPlaybookSteps,
    searchCollectionEntities,
    sendRequest,
    updateEnvironment,
    updatePlaybook,
    updatePlaybookStep
  } from "$lib/api/commands";
  import {
    type CollectionSearchResult,
    type EnvironmentDetail,
    type PlaybookDetail,
    type PlaybookRunStep,
    type PlaybookRunStatus,
    type PlaybookRunSummary,
    type PlaybookStep,
    type PlaybookSummary,
    type RequestDraft,
    type ScriptTestResult
  } from "$lib/api/types";
  import { runPreRequestScript, runTestScript } from "$lib/request-scripts";
  import { notifications } from "$lib/stores/notifications.svelte";

  type LiveStepState = {
    stepId: string;
    label: string;
    status: "queued" | "running" | "passed" | "failed" | "skipped" | "canceled";
    detail: string;
    statusCode: number | null;
    durationMs: number;
    testPassedCount: number;
    testFailedCount: number;
  };

  let playbooks = $state<PlaybookSummary[]>([]);
  let selectedPlaybookId = $state("");
  let selectedPlaybook = $state<PlaybookDetail | null>(null);
  let runs = $state<PlaybookRunSummary[]>([]);
  let liveSteps = $state<LiveStepState[]>([]);
  let addSearchQuery = $state("");
  let addSearchResults = $state<CollectionSearchResult[]>([]);
  let selectedRunDetail = $state<{ run: PlaybookRunSummary; steps: PlaybookRunStep[] } | null>(null);
  let activeEnvironmentDetail = $state<EnvironmentDetail | null>(null);
  let isLoading = $state(true);
  let isSavingPlaybook = $state(false);
  let isSearching = $state(false);
  let isRunning = $state(false);
  let cancelRequested = $state(false);
  let currentStepId = $state("");
  let runStartedAt = $state<number | null>(null);
  let elapsedNow = $state(Date.now());
  let errorText = $state("");

  let editableName = $state("");
  let editableDescription = $state("");
  let editableDefaultDelayMs = $state(0);
  let editableStopOnFailure = $state(true);
  let editableFailOnHttpError = $state(true);

  let runTimer: ReturnType<typeof setInterval> | null = null;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  let elapsedMs = $derived(runStartedAt ? elapsedNow - runStartedAt : 0);
  let enabledSteps = $derived((selectedPlaybook?.steps ?? []).filter((step) => step.enabled));
  let canRun = $derived(Boolean(selectedPlaybook && selectedPlaybook.steps.some((step) => step.enabled) && !isRunning));

  onMount(() => {
    void initialize();
    return () => {
      stopElapsedTimer();
      if (searchTimer) {
        clearTimeout(searchTimer);
      }
    };
  });

  async function initialize() {
    isLoading = true;
    try {
      await Promise.all([loadPlaybooks(), loadActiveEnvironment()]);
    } finally {
      isLoading = false;
    }
  }

  async function loadPlaybooks(preferredId = selectedPlaybookId) {
    try {
      const fetched = await listPlaybooks();
      playbooks = fetched;
      const nextId = preferredId && fetched.some((item) => item.id === preferredId)
        ? preferredId
        : fetched[0]?.id ?? "";
      selectedPlaybookId = nextId;
      if (nextId) {
        await loadPlaybook(nextId);
      } else {
        selectedPlaybook = null;
        runs = [];
      }
      errorText = "";
    } catch (error) {
      errorText = normalizeError(error);
    }
  }

  async function loadPlaybook(playbookId: string) {
    const detail = await getPlaybook(playbookId);
    selectedPlaybook = detail;
    selectedPlaybookId = detail.id;
    syncEditableFields(detail);
    runs = await listPlaybookRuns(detail.id, 20);
    selectedRunDetail = null;
  }

  async function loadActiveEnvironment() {
    const environments = await listEnvironments();
    const active = environments.find((environment) => environment.isActive);
    activeEnvironmentDetail = active ? await getEnvironment(active.id) : null;
  }

  function syncEditableFields(playbook: PlaybookDetail) {
    editableName = playbook.name;
    editableDescription = playbook.description;
    editableDefaultDelayMs = playbook.defaultDelayMs;
    editableStopOnFailure = playbook.stopOnFailure;
    editableFailOnHttpError = playbook.failOnHttpError;
  }

  async function handleCreatePlaybook() {
    const created = await createPlaybook({
      name: "Untitled playbook",
      description: "",
      defaultDelayMs: 0,
      stopOnFailure: true,
      failOnHttpError: true
    });
    notifications.success(created.name, "Playbook created");
    await loadPlaybooks(created.id);
  }

  async function handleSelectPlaybook(playbookId: string) {
    if (isRunning || playbookId === selectedPlaybookId) {
      return;
    }
    await loadPlaybook(playbookId);
  }

  async function handleSavePlaybook() {
    if (!selectedPlaybook) {
      return;
    }
    isSavingPlaybook = true;
    try {
      const saved = await updatePlaybook(selectedPlaybook.id, {
        name: editableName.trim(),
        description: editableDescription.trim(),
        defaultDelayMs: normalizeDelay(editableDefaultDelayMs),
        stopOnFailure: editableStopOnFailure,
        failOnHttpError: editableFailOnHttpError
      });
      selectedPlaybook = saved;
      syncEditableFields(saved);
      await loadPlaybooks(saved.id);
      notifications.success(saved.name, "Playbook saved");
    } catch (error) {
      notifications.error(normalizeError(error), "Save failed");
    } finally {
      isSavingPlaybook = false;
    }
  }

  async function handleDuplicatePlaybook() {
    if (!selectedPlaybook) {
      return;
    }
    const copied = await duplicatePlaybook(selectedPlaybook.id);
    notifications.success(copied.name, "Playbook duplicated");
    await loadPlaybooks(copied.id);
  }

  async function handleDeletePlaybook() {
    if (!selectedPlaybook || !window.confirm("Delete this playbook and its run logs?")) {
      return;
    }
    const deletedName = selectedPlaybook.name;
    await deletePlaybook(selectedPlaybook.id);
    notifications.success(deletedName, "Playbook deleted");
    await loadPlaybooks("");
  }

  function queueSearch() {
    if (searchTimer) {
      clearTimeout(searchTimer);
    }
    searchTimer = setTimeout(() => {
      void runSearch();
    }, 180);
  }

  async function runSearch() {
    const query = addSearchQuery.trim();
    if (!query) {
      addSearchResults = [];
      return;
    }
    isSearching = true;
    try {
      addSearchResults = (await searchCollectionEntities(query, 30)).filter((item) => item.kind === "request");
    } catch (error) {
      notifications.error(normalizeError(error), "Search failed");
    } finally {
      isSearching = false;
    }
  }

  async function handleAddStep(result: CollectionSearchResult) {
    if (!selectedPlaybook || result.kind !== "request") {
      return;
    }
    const step = await addPlaybookStep(selectedPlaybook.id, {
      savedRequestId: result.id,
      nameOverride: "",
      notes: "",
      enabled: true,
      delayAfterMs: null
    });
    selectedPlaybook = {
      ...selectedPlaybook,
      steps: [...selectedPlaybook.steps, step]
    };
    addSearchQuery = "";
    addSearchResults = [];
    notifications.success(step.savedRequestName, "Step added");
    await refreshSelectedPlaybook();
  }

  async function handleStepEnabled(step: PlaybookStep, enabled: boolean) {
    await saveStep(step, { ...step, enabled });
  }

  async function handleStepDelay(step: PlaybookStep, value: number) {
    await saveStep(step, { ...step, delayAfterMs: Number.isFinite(value) ? normalizeDelay(value) : null });
  }

  async function handleStepName(step: PlaybookStep, value: string) {
    await saveStep(step, { ...step, nameOverride: value });
  }

  async function saveStep(original: PlaybookStep, next: PlaybookStep) {
    if (!selectedPlaybook) {
      return;
    }
    const optimistic = selectedPlaybook.steps.map((step) => step.id === original.id ? next : step);
    selectedPlaybook = { ...selectedPlaybook, steps: optimistic };
    try {
      const saved = await updatePlaybookStep(original.id, {
        nameOverride: next.nameOverride,
        notes: next.notes,
        enabled: next.enabled,
        delayAfterMs: next.delayAfterMs ?? null
      });
      selectedPlaybook = {
        ...selectedPlaybook,
        steps: selectedPlaybook.steps.map((step) => step.id === saved.id ? saved : step)
      };
    } catch (error) {
      selectedPlaybook = {
        ...selectedPlaybook,
        steps: selectedPlaybook.steps.map((step) => step.id === original.id ? original : step)
      };
      notifications.error(normalizeError(error), "Step save failed");
    }
  }

  async function moveStep(step: PlaybookStep, direction: -1 | 1) {
    if (!selectedPlaybook) {
      return;
    }
    const index = selectedPlaybook.steps.findIndex((item) => item.id === step.id);
    const targetIndex = index + direction;
    if (index < 0 || targetIndex < 0 || targetIndex >= selectedPlaybook.steps.length) {
      return;
    }
    const next = [...selectedPlaybook.steps];
    const [moved] = next.splice(index, 1);
    next.splice(targetIndex, 0, moved);
    selectedPlaybook = { ...selectedPlaybook, steps: next };
    try {
      const reordered = await reorderPlaybookSteps(selectedPlaybook.id, {
        stepIds: next.map((item) => item.id)
      });
      selectedPlaybook = { ...selectedPlaybook, steps: reordered };
    } catch (error) {
      notifications.error(normalizeError(error), "Reorder failed");
      await refreshSelectedPlaybook();
    }
  }

  async function handleDeleteStep(step: PlaybookStep) {
    if (!selectedPlaybook || !window.confirm("Remove this step from the playbook?")) {
      return;
    }
    await deletePlaybookStep(step.id);
    selectedPlaybook = {
      ...selectedPlaybook,
      steps: selectedPlaybook.steps.filter((item) => item.id !== step.id)
    };
    notifications.success(displayStepName(step), "Step removed");
    await refreshSelectedPlaybook();
  }

  async function refreshSelectedPlaybook() {
    if (selectedPlaybook) {
      await loadPlaybook(selectedPlaybook.id);
    }
  }

  async function handleRunPlaybook() {
    if (!selectedPlaybook || !canRun) {
      return;
    }

    const stepsToRun = selectedPlaybook.steps.filter((step) => step.enabled);
    isRunning = true;
    cancelRequested = false;
    currentStepId = "";
    liveSteps = stepsToRun.map((step) => ({
      stepId: step.id,
      label: displayStepName(step),
      status: "queued",
      detail: "",
      statusCode: null,
      durationMs: 0,
      testPassedCount: 0,
      testFailedCount: 0
    }));
    runStartedAt = Date.now();
    startElapsedTimer();

    let run: PlaybookRunSummary | null = null;
    let failed = false;
    let stoppedReason = "";

    try {
      run = await createPlaybookRun({
        playbookId: selectedPlaybook.id,
        totalSteps: stepsToRun.length
      });

      for (let index = 0; index < stepsToRun.length; index += 1) {
        const step = stepsToRun[index];
        if (cancelRequested) {
          stoppedReason = "Canceled by user.";
          await recordRemainingSkipped(run.id, stepsToRun.slice(index), "Canceled before this step ran.");
          break;
        }

        const result = await executeStep(run.id, step);
        if (result.failed) {
          failed = true;
          stoppedReason = result.reason;
          if (selectedPlaybook.stopOnFailure) {
            await recordRemainingSkipped(run.id, stepsToRun.slice(index + 1), "Skipped after an earlier failure.");
            break;
          }
        }

        const delayMs = step.delayAfterMs ?? selectedPlaybook.defaultDelayMs;
        if (delayMs > 0 && index < stepsToRun.length - 1 && !cancelRequested) {
          const liveStep = liveSteps.find((item) => item.stepId === step.id);
          if (liveStep?.status !== "failed" && liveStep?.status !== "canceled") {
            updateLiveStep(step.id, { detail: `Waiting ${formatDuration(delayMs)} before next step.` });
          }
          await waitForDelay(delayMs);
        }
      }

      const status: PlaybookRunStatus = cancelRequested ? "canceled" : failed ? "failed" : "passed";
      const finished = await finishPlaybookRun(run.id, {
        status,
        stoppedReason,
        totalDurationMs: runStartedAt ? Date.now() - runStartedAt : 0
      });
      const message = status === "passed" ? "Every enabled step completed." : stoppedReason || "The playbook run stopped.";
      const title = status === "passed" ? "Playbook passed" : status === "canceled" ? "Playbook canceled" : "Playbook failed";
      if (status === "passed") {
        notifications.success(message, title);
      } else if (status === "canceled") {
        notifications.warning(message, title);
      } else {
        notifications.error(message, title);
      }
      runs = [finished, ...runs.filter((item) => item.id !== finished.id)].slice(0, 20);
    } catch (error) {
      const message = normalizeError(error);
      notifications.error(message, "Run failed");
      if (run) {
        await finishPlaybookRun(run.id, {
          status: "failed",
          stoppedReason: message,
          totalDurationMs: runStartedAt ? Date.now() - runStartedAt : 0
        });
      }
    } finally {
      isRunning = false;
      cancelRequested = false;
      currentStepId = "";
      stopElapsedTimer();
      if (selectedPlaybook) {
        runs = await listPlaybookRuns(selectedPlaybook.id, 20);
      }
    }
  }

  async function executeStep(runId: string, step: PlaybookStep): Promise<{ failed: boolean; reason: string }> {
    currentStepId = step.id;
    updateLiveStep(step.id, { status: "running", detail: "Preparing request." });

    if (step.missingSavedRequest || !step.savedRequestId) {
      const reason = "Linked saved request is missing.";
      await recordStepFailure(runId, step, reason, null, 0, 0, 0, 0, "");
      updateLiveStep(step.id, { status: "failed", detail: reason });
      return { failed: true, reason };
    }

    try {
      const context = await getPlaybookExecutionContext(step.id);
      const requestToSend = cloneRequestDraft(context.savedRequest.request);
      const prepared = await runPreRequestScript(
        requestToSend,
        activeEnvironmentDetail?.variables ?? [],
        context.inheritedScripts,
        {
          activeEnvironment: activeEnvironmentDetail,
          persistActiveEnvironment: persistActiveEnvironmentFromScript
        }
      );

      if (prepared.errorText) {
        await recordStepFailure(runId, step, prepared.errorText, null, 0, 0, 0, 0, "");
        updateLiveStep(step.id, { status: "failed", detail: prepared.errorText });
        return { failed: true, reason: prepared.errorText };
      }

      updateLiveStep(step.id, { detail: "Sending request." });
      const sendResult = await sendRequest(prepared.request);
      const scriptExecution = await runTestScript(
        requestToSend,
        sendResult.response,
        activeEnvironmentDetail?.variables ?? [],
        context.inheritedScripts,
        {
          activeEnvironment: activeEnvironmentDetail,
          persistActiveEnvironment: persistActiveEnvironmentFromScript
        }
      );

      const failedTests = scriptExecution.tests.filter((test) => test.status === "failed");
      const passedTests = scriptExecution.tests.filter((test) => test.status === "passed");
      const httpFailed =
        selectedPlaybook?.failOnHttpError &&
        (!sendResult.response.statusCode ||
          sendResult.response.statusCode < 200 ||
          sendResult.response.statusCode >= 400);
      const failed =
        Boolean(scriptExecution.testScriptErrorText) || failedTests.length > 0 || Boolean(httpFailed);
      const reason =
        scriptExecution.testScriptErrorText ||
        (failedTests.length > 0 ? formatFailedTests(failedTests) : "") ||
        (httpFailed ? `HTTP ${sendResult.response.statusCode ?? "no status"} treated as failure.` : "");

      await recordPlaybookRunStep(runId, {
        stepId: step.id,
        savedRequestId: context.savedRequest.id,
        savedRequestName: context.savedRequest.name,
        method: context.savedRequest.request.method,
        url: context.savedRequest.request.url,
        status: failed ? "failed" : "passed",
        statusCode: sendResult.response.statusCode,
        durationMs: sendResult.response.durationMs,
        responseSizeBytes: sendResult.response.sizeBytes,
        testPassedCount: passedTests.length,
        testFailedCount: failedTests.length,
        testErrorText: scriptExecution.testScriptErrorText,
        errorText: reason
      });

      updateLiveStep(step.id, {
        status: failed ? "failed" : "passed",
        detail: failed ? reason : "Completed.",
        statusCode: sendResult.response.statusCode,
        durationMs: sendResult.response.durationMs,
        testPassedCount: passedTests.length,
        testFailedCount: failedTests.length
      });

      if (sendResult.historyPersistenceError) {
        notifications.warning(sendResult.historyPersistenceError, "History not saved");
      }

      return { failed, reason };
    } catch (error) {
      const message = normalizeError(error);
      if (cancelRequested) {
        await recordPlaybookRunStep(runId, {
          stepId: step.id,
          savedRequestId: step.savedRequestId ?? null,
          savedRequestName: displayStepName(step),
          method: step.method ?? "",
          url: step.url ?? "",
          status: "canceled",
          statusCode: null,
          durationMs: 0,
          responseSizeBytes: 0,
          testPassedCount: 0,
          testFailedCount: 0,
          testErrorText: "",
          errorText: "Canceled by user."
        });
        updateLiveStep(step.id, { status: "canceled", detail: "Canceled by user." });
        return { failed: true, reason: "Canceled by user." };
      }

      await recordStepFailure(runId, step, message, null, 0, 0, 0, 0, "");
      updateLiveStep(step.id, { status: "failed", detail: message });
      return { failed: true, reason: message };
    }
  }

  async function recordStepFailure(
    runId: string,
    step: PlaybookStep,
    errorText: string,
    statusCode: number | null,
    durationMs: number,
    responseSizeBytes: number,
    testPassedCount: number,
    testFailedCount: number,
    testErrorText: string
  ) {
    await recordPlaybookRunStep(runId, {
      stepId: step.id,
      savedRequestId: step.savedRequestId ?? null,
      savedRequestName: displayStepName(step),
      method: step.method ?? "",
      url: step.url ?? "",
      status: "failed",
      statusCode,
      durationMs,
      responseSizeBytes,
      testPassedCount,
      testFailedCount,
      testErrorText,
      errorText
    });
  }

  async function recordRemainingSkipped(runId: string, steps: PlaybookStep[], reason: string) {
    for (const step of steps) {
      await recordPlaybookRunStep(runId, {
        stepId: step.id,
        savedRequestId: step.savedRequestId ?? null,
        savedRequestName: displayStepName(step),
        method: step.method ?? "",
        url: step.url ?? "",
        status: "skipped",
        statusCode: null,
        durationMs: 0,
        responseSizeBytes: 0,
        testPassedCount: 0,
        testFailedCount: 0,
        testErrorText: "",
        errorText: reason
      });
      updateLiveStep(step.id, { status: "skipped", detail: reason });
    }
  }

  async function handleCancelRun() {
    if (!isRunning) {
      return;
    }
    cancelRequested = true;
    await cancelActiveRequest();
  }

  async function persistActiveEnvironmentFromScript(nextEnvironment: EnvironmentDetail): Promise<EnvironmentDetail> {
    const updated = await updateEnvironment(nextEnvironment.id, {
      name: nextEnvironment.name.trim(),
      variables: nextEnvironment.variables
    });
    activeEnvironmentDetail = updated;
    return updated;
  }

  function updateLiveStep(stepId: string, patch: Partial<LiveStepState>) {
    liveSteps = liveSteps.map((step) => step.stepId === stepId ? { ...step, ...patch } : step);
  }

  async function waitForDelay(delayMs: number) {
    const started = Date.now();
    while (!cancelRequested && Date.now() - started < delayMs) {
      await new Promise((resolveDelay) => setTimeout(resolveDelay, Math.min(150, delayMs)));
    }
  }

  function startElapsedTimer() {
    stopElapsedTimer();
    elapsedNow = Date.now();
    runTimer = setInterval(() => {
      elapsedNow = Date.now();
    }, 250);
  }

  function stopElapsedTimer() {
    if (runTimer) {
      clearInterval(runTimer);
      runTimer = null;
    }
  }

  function displayStepName(step: PlaybookStep) {
    return step.nameOverride.trim() || step.savedRequestName || "Missing request";
  }

  function normalizeDelay(value: number) {
    return Math.max(0, Math.min(3_600_000, Math.round(Number(value) || 0)));
  }

  function normalizeError(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }

  function cloneRequestDraft(request: RequestDraft): RequestDraft {
    return JSON.parse(JSON.stringify(request)) as RequestDraft;
  }

  function formatFailedTests(tests: ScriptTestResult[]) {
    const first = tests[0];
    return `${tests.length} test${tests.length === 1 ? "" : "s"} failed${first?.name ? `: ${first.name}` : ""}.`;
  }

  function formatDuration(ms: number) {
    if (ms < 1000) {
      return `${ms} ms`;
    }
    return `${(ms / 1000).toFixed(ms % 1000 === 0 ? 0 : 1)} s`;
  }

  function formatDate(value: string | null | undefined) {
    return value ? new Date(value).toLocaleString() : "Running";
  }

  function statusLabel(status: string) {
    return status.charAt(0).toUpperCase() + status.slice(1);
  }

  async function openSavedRequest(step: PlaybookStep) {
    if (step.savedRequestId) {
      await goto(resolve(`/?savedRequestId=${encodeURIComponent(step.savedRequestId)}`));
    }
  }

  async function toggleRunDetail(run: PlaybookRunSummary) {
    if (selectedRunDetail?.run.id === run.id) {
      selectedRunDetail = null;
      return;
    }

    const detail = await getPlaybookRun(run.id);
    selectedRunDetail = { run: detail, steps: detail.steps };
  }
</script>

<svelte:head>
  <title>Playbooks - PostNot</title>
</svelte:head>

<div class="playbooks-page">
  <section class="panel playbook-list-panel">
    <div class="playbook-section-header">
      <div>
        <p class="eyebrow">Sequences</p>
        <h2>Playbooks</h2>
      </div>
      <button class="button-secondary button-compact" type="button" onclick={handleCreatePlaybook}>New</button>
    </div>

    {#if isLoading}
      <p class="muted-text">Loading playbooks...</p>
    {:else if errorText}
      <p class="error-text">{errorText}</p>
    {:else if playbooks.length === 0}
      <div class="empty-state">
        <h3>No playbooks yet</h3>
        <p>Create a playbook to run saved requests in order.</p>
      </div>
    {:else}
      <div class="playbook-list">
        {#each playbooks as playbook}
          <button
            class={["playbook-list-item", playbook.id === selectedPlaybookId && "playbook-list-item-active"]}
            type="button"
            onclick={() => handleSelectPlaybook(playbook.id)}
            disabled={isRunning}
          >
            <span>{playbook.name}</span>
            <small>{playbook.stepCount} step{playbook.stepCount === 1 ? "" : "s"}</small>
          </button>
        {/each}
      </div>
    {/if}
  </section>

  <section class="panel playbook-editor-panel">
    {#if selectedPlaybook}
      <div class="playbook-editor-header">
        <div>
          <p class="eyebrow">Editor</p>
          <h2>{selectedPlaybook.name}</h2>
        </div>
        <div class="playbook-header-actions">
          <button class="button-secondary button-compact" type="button" onclick={handleDuplicatePlaybook} disabled={isRunning}>Duplicate</button>
          <button class="button-danger button-compact" type="button" onclick={handleDeletePlaybook} disabled={isRunning}>Delete</button>
          <button class="button-primary button-compact" type="button" onclick={handleSavePlaybook} disabled={isSavingPlaybook || isRunning}>
            {isSavingPlaybook ? "Saving..." : "Save"}
          </button>
        </div>
      </div>

      <div class="playbook-settings-grid">
        <label>
          <span>Name</span>
          <input bind:value={editableName} disabled={isRunning} />
        </label>
        <label>
          <span>Default delay (ms)</span>
          <input type="number" min="0" max="3600000" step="100" bind:value={editableDefaultDelayMs} disabled={isRunning} />
        </label>
        <label class="wide-field">
          <span>Description</span>
          <textarea rows="2" bind:value={editableDescription} disabled={isRunning}></textarea>
        </label>
        <label class="toggle-row">
          <input class="playbook-checkbox" type="checkbox" bind:checked={editableStopOnFailure} disabled={isRunning} />
          <span>Stop on failure</span>
        </label>
        <label class="toggle-row">
          <input class="playbook-checkbox" type="checkbox" bind:checked={editableFailOnHttpError} disabled={isRunning} />
          <span>Treat non-2xx/3xx status as failure</span>
        </label>
      </div>

      <div class="add-step-box">
        <label>
          <span>Add saved request</span>
          <input
            placeholder="Search collections and requests"
            bind:value={addSearchQuery}
            oninput={queueSearch}
            disabled={isRunning}
          />
        </label>
        {#if isSearching}
          <p class="muted-text">Searching...</p>
        {:else if addSearchResults.length > 0}
          <div class="search-results">
            {#each addSearchResults as result}
              <button type="button" onclick={() => handleAddStep(result)} disabled={isRunning}>
                <span><strong>{result.method}</strong> {result.name}</span>
                <small>{result.collectionName}{#if result.ancestorNames.length} / {result.ancestorNames.join(" / ")}{/if}</small>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="steps-list">
        {#if selectedPlaybook.steps.length === 0}
          <div class="empty-state">
            <h3>No steps</h3>
            <p>Search for saved requests above and add them to this playbook.</p>
          </div>
        {:else}
          {#each selectedPlaybook.steps as step, index}
            <article
              class={[
                "step-row",
                currentStepId === step.id && "step-row-running",
                !step.enabled && "step-row-disabled",
                step.missingSavedRequest && "step-row-missing"
              ]}
            >
              <div class="step-order-controls">
                <button
                  class="step-order-button step-order-button-up"
                  type="button"
                  title="Move step up"
                  aria-label="Move step up"
                  onclick={() => moveStep(step, -1)}
                  disabled={isRunning || index === 0}
                ></button>
                <div class="step-index">{index + 1}</div>
                <button
                  class="step-order-button step-order-button-down"
                  type="button"
                  title="Move step down"
                  aria-label="Move step down"
                  onclick={() => moveStep(step, 1)}
                  disabled={isRunning || index === selectedPlaybook.steps.length - 1}
                ></button>
                <label
                  class="step-toggle step-toggle-rail"
                  title={step.enabled ? "Disable this step" : "Enable this step"}
                  aria-label={step.enabled ? "Disable this step" : "Enable this step"}
                >
                  <input
                    class="playbook-checkbox"
                    type="checkbox"
                    checked={step.enabled}
                    onchange={(event) => handleStepEnabled(step, event.currentTarget.checked)}
                    disabled={isRunning || step.missingSavedRequest}
                  />
                </label>
              </div>
              <div class="step-main">
                <div class="step-title-row">
                  <div>
                    <h3>{displayStepName(step)}</h3>
                    <p>{step.method ?? "--"} {step.url ?? "Linked request missing"}</p>
                  </div>
                </div>
                <div class="step-fields">
                  <label>
                    <span>Name override</span>
                    <input
                      value={step.nameOverride}
                      onblur={(event) => handleStepName(step, event.currentTarget.value)}
                      disabled={isRunning}
                    />
                  </label>
                  <label>
                    <span>Delay after (ms)</span>
                    <input
                      type="number"
                      min="0"
                      max="3600000"
                      step="100"
                      value={step.delayAfterMs ?? ""}
                      placeholder={`${selectedPlaybook.defaultDelayMs}`}
                      onblur={(event) => handleStepDelay(step, Number(event.currentTarget.value || 0))}
                      disabled={isRunning}
                    />
                  </label>
                </div>
              </div>
              <div class="step-actions">
                <button class="button-secondary button-compact" type="button" onclick={() => openSavedRequest(step)} disabled={!step.savedRequestId}>Open</button>
                <button
                  class="icon-button row-action-button row-action-danger"
                  type="button"
                  title={`Remove ${displayStepName(step)} from playbook`}
                  aria-label={`Remove ${displayStepName(step)} from playbook`}
                  onclick={() => handleDeleteStep(step)}
                  disabled={isRunning}
                >
                  <svg viewBox="0 0 20 20" aria-hidden="true">
                    <path d="M3 5h14" />
                    <path d="M8 5V3h4v2" />
                    <path d="M6 8v8" />
                    <path d="M10 8v8" />
                    <path d="M14 8v8" />
                    <path d="M5 5l1 12h8l1-12" />
                  </svg>
                </button>
              </div>
            </article>
          {/each}
        {/if}
      </div>
    {:else}
      <div class="empty-state">
        <h2>Select a playbook</h2>
        <p>Create or select a playbook to build a request sequence.</p>
      </div>
    {/if}
  </section>

  <section class="panel playbook-run-panel">
    <div class="playbook-section-header">
      <div>
        <p class="eyebrow">Run</p>
        <h2>Execution</h2>
      </div>
      {#if isRunning}
        <button class="button-danger button-compact" type="button" onclick={handleCancelRun}>Cancel</button>
      {:else}
        <button class="button-primary button-compact" type="button" onclick={handleRunPlaybook} disabled={!canRun}>Run</button>
      {/if}
    </div>

    <div class="run-summary">
      <div>
        <span>{enabledSteps.length}</span>
        <small>enabled</small>
      </div>
      <div>
        <span>{formatDuration(elapsedMs)}</span>
        <small>elapsed</small>
      </div>
      <div>
        <span>{activeEnvironmentDetail?.name ?? "None"}</span>
        <small>environment</small>
      </div>
    </div>

    {#if liveSteps.length > 0}
      <div class="live-steps">
        {#each liveSteps as step}
          <div class={["live-step", `live-step-${step.status}`]}>
            <div>
              <strong>{step.label}</strong>
              <span>{statusLabel(step.status)}{#if step.statusCode} / HTTP {step.statusCode}{/if}</span>
            </div>
            <p>{step.detail || (step.durationMs ? formatDuration(step.durationMs) : "")}</p>
            {#if step.testPassedCount || step.testFailedCount}
              <small>{step.testPassedCount} passed, {step.testFailedCount} failed</small>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    <div class="run-history">
      <h3>Run log</h3>
      {#if runs.length === 0}
        <p class="muted-text">No grouped playbook runs yet.</p>
      {:else}
        {#each runs as run}
          <button
            class={["run-history-item", selectedRunDetail?.run.id === run.id && "run-history-item-active"]}
            type="button"
            onclick={() => toggleRunDetail(run)}
          >
            <span>{statusLabel(run.status)} · {formatDate(run.startedAt)}</span>
            <small>{run.passedSteps} passed / {run.failedSteps} failed / {run.skippedSteps} skipped</small>
          </button>
          {#if selectedRunDetail?.run.id === run.id}
            <div class="run-history-detail">
              <p>{run.stoppedReason || "Completed without a stop reason."}</p>
              <p>{formatDuration(run.totalDurationMs)} total</p>
              {#each selectedRunDetail.steps as step}
                <div class={["run-detail-step", step.errorText && "run-detail-step-error"]}>
                  <p><strong>{statusLabel(step.status)}</strong> {step.method} {step.savedRequestName}{#if step.statusCode} / HTTP {step.statusCode}{/if}</p>
                  {#if step.errorText}
                    <p class="run-error-text">{step.errorText}</p>
                  {/if}
                  {#if step.testErrorText}
                    <p class="run-error-text">{step.testErrorText}</p>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  </section>
</div>

<style>
  :global(.workspace:has(.playbooks-page)) {
    overflow: hidden;
  }

  .playbooks-page {
    display: grid;
    grid-template-columns: minmax(210px, 0.72fr) minmax(420px, 1.7fr) minmax(300px, 0.95fr);
    gap: 18px;
    height: 100%;
    max-height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .playbook-list-panel,
  .playbook-editor-panel,
  .playbook-run-panel {
    padding: 16px;
    min-height: 0;
    height: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .playbook-section-header,
  .playbook-editor-header,
  .step-title-row,
  .playbook-header-actions,
  .step-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .playbook-section-header,
  .playbook-editor-header {
    justify-content: space-between;
  }

  .playbook-section-header h2,
  .playbook-editor-header h2,
  .empty-state h2,
  .empty-state h3,
  .step-main h3,
  .run-history h3 {
    margin: 0;
  }

  .playbook-list,
  .steps-list,
  .live-steps,
  .run-history {
    min-height: 0;
    overflow-y: auto;
  }

  .playbook-list {
    display: grid;
    align-content: start;
    gap: 6px;
  }

  .steps-list {
    flex: 1 1 auto;
    align-content: start;
  }

  .live-steps {
    flex: 0 1 auto;
    max-height: 32%;
  }

  .run-history {
    flex: 1 1 auto;
    align-content: start;
  }

  .playbook-list-item,
  .run-history-item,
  .search-results button {
    width: 100%;
    text-align: left;
    border: 1px solid var(--border-soft);
    background: var(--surface-subtle);
    border-radius: var(--radius-sm);
    padding: 8px 10px;
    display: grid;
    gap: 2px;
  }

  .playbook-list-item span {
    font-size: 0.92rem;
    line-height: 1.2;
  }

  .playbook-list-item small {
    font-size: 0.78rem;
    line-height: 1.2;
  }

  .playbook-list-item-active,
  .run-history-item-active {
    border-color: var(--bg-accent);
    background: var(--bg-accent-soft);
  }

  .playbook-list-item small,
  .run-history-item small,
  .search-results small,
  .muted-text,
  .empty-state p,
  .step-main p,
  .live-step span,
  .live-step small,
  .run-history-detail {
    color: var(--text-secondary);
  }

  .playbook-settings-grid,
  .step-fields {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 150px;
    gap: 8px 10px;
  }

  .step-fields {
    grid-template-columns: minmax(180px, 0.48fr) 140px;
    align-items: end;
  }

  label {
    display: grid;
    gap: 5px;
    font-size: 0.82rem;
    color: var(--text-secondary);
  }

  input,
  textarea {
    width: 100%;
    border: 1px solid var(--border-soft);
    background: var(--control-bg);
    color: var(--text-primary);
    border-radius: var(--radius-sm);
    padding: 8px 10px;
    min-width: 0;
  }

  textarea {
    resize: vertical;
  }

  .wide-field {
    grid-column: 1 / -1;
  }

  .toggle-row,
  .step-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .playbook-checkbox {
    appearance: none;
    -webkit-appearance: none;
    width: 18px;
    height: 18px;
    min-width: 18px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 5px;
    background: var(--control-bg);
    display: grid;
    place-items: center;
  }

  .playbook-checkbox::before {
    content: "";
    width: 8px;
    height: 5px;
    border-left: 2px solid white;
    border-bottom: 2px solid white;
    transform: rotate(-45deg) scale(0);
    transform-origin: center;
  }

  .playbook-checkbox:checked {
    border-color: var(--bg-accent);
    background: var(--bg-accent);
  }

  .playbook-checkbox:checked::before {
    transform: rotate(-45deg) scale(1);
  }

  .playbook-checkbox:disabled {
    opacity: 0.55;
  }

  .add-step-box,
  .run-summary,
  .empty-state {
    border: 1px solid var(--border-soft);
    background: var(--surface-subtle);
    border-radius: var(--radius-sm);
    padding: 10px;
  }

  .search-results {
    display: grid;
    gap: 6px;
    max-height: 220px;
    overflow-y: auto;
  }

  .steps-list {
    display: grid;
    gap: 8px;
  }

  .step-row {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto;
    gap: 10px;
    padding: 10px;
    border: 1px solid var(--border-soft);
    background: var(--surface);
    border-radius: var(--radius-sm);
  }

  .step-row-running {
    border-color: var(--bg-accent);
  }

  .step-row-disabled {
    background: var(--surface-subtle);
  }

  .step-row-disabled .step-main {
    opacity: 0.58;
  }

  .step-row-disabled .step-index {
    background: var(--surface-subtle);
    border: 1px dashed var(--border-strong);
  }

  .step-row-missing {
    border-color: var(--danger);
    background: var(--surface-danger-soft);
  }

  .step-index {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--surface-muted);
    color: var(--text-secondary);
    font-weight: 700;
  }

  .step-order-controls {
    display: grid;
    justify-items: center;
    align-content: start;
    gap: 4px;
  }

  .step-order-button {
    width: 24px;
    height: 20px;
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-sm);
    background: var(--surface-muted);
    display: grid;
    place-items: center;
  }

  .step-order-button::before {
    content: "";
    width: 7px;
    height: 7px;
    border-left: 2px solid currentColor;
    border-top: 2px solid currentColor;
  }

  .step-order-button-up::before {
    transform: translateY(2px) rotate(45deg);
  }

  .step-order-button-down::before {
    transform: translateY(-2px) rotate(225deg);
  }

  .step-order-button:disabled {
    opacity: 0.35;
  }

  .step-toggle-rail {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    margin-top: 2px;
  }

  .step-main {
    min-width: 0;
    display: grid;
    gap: 8px;
  }

  .step-main p {
    margin: 3px 0 0;
    overflow-wrap: anywhere;
  }

  .step-title-row {
    justify-content: space-between;
    align-items: flex-start;
  }

  .step-title-row > div {
    min-width: 0;
  }

  .step-actions {
    align-self: start;
    flex-wrap: wrap;
    justify-content: flex-end;
    min-width: 0;
    max-width: 150px;
  }

  .run-summary {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  .run-summary div {
    display: grid;
    gap: 2px;
  }

  .run-summary span {
    font-weight: 800;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .run-summary small {
    color: var(--text-muted);
  }

  .live-steps {
    display: grid;
    gap: 8px;
  }

  .live-step {
    border: 1px solid var(--border-soft);
    border-left: 4px solid var(--border-strong);
    border-radius: var(--radius-sm);
    padding: 10px;
    background: var(--surface-subtle);
  }

  .live-step-running {
    border-left-color: var(--bg-accent);
  }

  .live-step-passed {
    border-left-color: var(--success);
  }

  .live-step-failed,
  .live-step-canceled {
    border-left-color: var(--danger);
  }

  .live-step-skipped {
    opacity: 0.75;
  }

  .live-step div {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }

  .live-step p {
    margin: 5px 0 0;
    overflow-wrap: anywhere;
  }

  .run-history {
    display: grid;
    gap: 8px;
  }

  .run-history-detail {
    padding: 0 10px 8px;
    font-size: 0.86rem;
  }

  .run-history-detail p {
    margin: 4px 0;
  }

  .run-detail-step {
    margin-top: 8px;
    padding: 8px;
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-sm);
    background: var(--surface-subtle);
  }

  .run-detail-step-error {
    border-left: 3px solid var(--danger);
  }

  .run-error-text {
    color: var(--danger);
    overflow-wrap: anywhere;
  }

  .error-text {
    color: var(--danger);
  }

  @media (max-width: 960px) {
    .playbooks-page {
      grid-template-columns: 1fr;
      height: 100%;
      max-height: 100%;
      min-height: 0;
      overflow: hidden;
    }

    .playbook-list-panel,
    .playbook-editor-panel,
    .playbook-run-panel {
      min-height: 0;
      overflow: hidden;
    }
  }

  @media (max-width: 1280px) {
    .step-row {
      grid-template-columns: 34px minmax(0, 1fr);
    }

    .step-actions {
      grid-column: 2;
      justify-content: flex-start;
      max-width: none;
    }
  }
</style>
