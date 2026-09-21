// ==UserScript==
// @name         Search Kernel Issue Autopilot
// @namespace    https://github.com/superkelvint/searchkernel
// @version      0.1.0
// @description  Pick one eligible Search Kernel issue, claim it, dispatch it to a Codex coding agent, and request an independent Codex review when its PR is ready.
// @match        https://github.com/superkelvint/searchkernel/issues*
// @match        https://chatgpt.com/*
// @connect      api.github.com
// @grant        GM_xmlhttpRequest
// @grant        GM_getValue
// @grant        GM_setValue
// @grant        GM_registerMenuCommand
// @grant        GM_openInTab
// @run-at       document-idle
// ==/UserScript==

(function () {
  "use strict";

  const DEFAULT_REPO = "superkelvint/searchkernel";
  const DEFAULT_ASSIGNEE = "superkelvint";
  const DEFAULT_AGENT_URL = "https://chatgpt.com/codex";
  const POLL_MS = 30_000;

  const CONFIG_KEY = "searchkernel-issue-autopilot-config-v1";
  const STATE_KEY = "searchkernel-issue-autopilot-state-v1";
  const HEALTH_KEY = "searchkernel-issue-autopilot-health-v1";

  const CLAIM_MARKER = "<!-- searchkernel-issue-autopilot-claim -->";
  const REVIEW_MARKER = "<!-- searchkernel-issue-autopilot-review -->";

  let cycleInFlight = false;

  function defaultConfig() {
    return {
      repo: DEFAULT_REPO,
      token: "",
      assignee: DEFAULT_ASSIGNEE,
      agentUrl: DEFAULT_AGENT_URL,
      enabled: false,
      excludedIssues: [21],
      blockedLabels: [
        "blocked",
        "do-not-work",
        "in-progress",
        "claimed",
        "needs-design",
        "waiting",
      ],
    };
  }

  function loadConfig() {
    return {
      ...defaultConfig(),
      ...GM_getValue(CONFIG_KEY, {}),
    };
  }

  function saveConfig(config) {
    GM_setValue(CONFIG_KEY, config);
    renderStatus();
  }

  function defaultState() {
    return {
      active: null,
      pendingAgentPrompt: null,
      lastCompleted: null,
    };
  }

  function loadState() {
    return {
      ...defaultState(),
      ...GM_getValue(STATE_KEY, {}),
    };
  }

  function saveState(patch) {
    const state = {
      ...loadState(),
      ...patch,
    };
    GM_setValue(STATE_KEY, state);
    renderStatus();
    return state;
  }

  function defaultHealth() {
    return {
      state: "IDLE",
      message: "",
      lastAttemptAt: null,
      lastSuccessAt: null,
      lastErrorAt: null,
      lastError: "",
    };
  }

  function loadHealth() {
    return {
      ...defaultHealth(),
      ...GM_getValue(HEALTH_KEY, {}),
    };
  }

  function saveHealth(patch) {
    const health = {
      ...loadHealth(),
      ...patch,
    };
    GM_setValue(HEALTH_KEY, health);
    renderStatus();
    return health;
  }

  function nowIso() {
    return new Date().toISOString();
  }

  function shortTime(value) {
    if (!value) return "never";
    try {
      return new Date(value).toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      });
    } catch {
      return value;
    }
  }

  function conciseError(error) {
    const text = error instanceof Error ? error.message : String(error);
    return text.length > 180 ? text.slice(0, 177) + "..." : text;
  }

  function normalizeUrl(url) {
    try {
      const parsed = new URL(url);
      return parsed.origin + parsed.pathname.replace(/\/$/, "");
    } catch {
      return "";
    }
  }

  function isGitHubIssuesPage() {
    return (
      location.hostname === "github.com" &&
      /^\/superkelvint\/searchkernel\/issues(?:\/|$)/.test(location.pathname)
    );
  }

  function isAgentPage(config) {
    const target = normalizeUrl(config.agentUrl);
    if (!target) return false;

    try {
      const current = new URL(location.href);
      const configured = new URL(target);
      const configuredPath = configured.pathname.replace(/\/$/, "");
      const currentPath = current.pathname.replace(/\/$/, "");
      return (
        current.origin === configured.origin &&
        (currentPath === configuredPath ||
          currentPath.startsWith(configuredPath + "/"))
      );
    } catch {
      return false;
    }
  }

  function githubRequest(method, pathOrUrl, token, body) {
    const url = pathOrUrl.startsWith("https://")
      ? pathOrUrl
      : `https://api.github.com${pathOrUrl}`;

    return new Promise((resolve, reject) => {
      GM_xmlhttpRequest({
        method,
        url,
        headers: {
          Accept: "application/vnd.github+json",
          Authorization: `Bearer ${token}`,
          "X-GitHub-Api-Version": "2022-11-28",
          ...(body ? { "Content-Type": "application/json" } : {}),
        },
        data: body ? JSON.stringify(body) : undefined,
        timeout: 20_000,
        onload(response) {
          if (response.status < 200 || response.status >= 300) {
            let detail = response.responseText || "";
            try {
              detail = JSON.parse(detail).message || detail;
            } catch {
              // Keep raw detail.
            }
            reject(
              new Error(
                `GitHub API ${response.status}${detail ? `: ${detail}` : ""}`
              )
            );
            return;
          }

          if (!response.responseText) {
            resolve(null);
            return;
          }

          try {
            resolve(JSON.parse(response.responseText));
          } catch (error) {
            reject(
              new Error(
                `GitHub returned invalid JSON: ${conciseError(error)}`
              )
            );
          }
        },
        onerror() {
          reject(new Error("GitHub API network error"));
        },
        ontimeout() {
          reject(new Error("GitHub API request timed out"));
        },
      });
    });
  }

  async function getIssue(number, config) {
    return githubRequest(
      "GET",
      `/repos/${config.repo}/issues/${number}`,
      config.token
    );
  }

  async function getIssueComments(number, config) {
    return githubRequest(
      "GET",
      `/repos/${config.repo}/issues/${number}/comments?per_page=100`,
      config.token
    );
  }

  async function getIssueTimeline(number, config) {
    return githubRequest(
      "GET",
      `/repos/${config.repo}/issues/${number}/timeline?per_page=100`,
      config.token
    );
  }

  function visibleIssueNumbers(config) {
    if (!isGitHubIssuesPage()) return [];

    const ownerRepo = config.repo.replace(/[.*+?^$()|[\]\\]/g, "\\$&");
    const pattern = new RegExp(
      `^/${ownerRepo.replace("/", "\\/")}\\/issues\\/(\\d+)$`
    );
    const seen = new Set();
    const numbers = [];

    for (const anchor of document.querySelectorAll('a[href*="/issues/"]')) {
      let pathname;
      try {
        pathname = new URL(anchor.href, location.href).pathname;
      } catch {
        continue;
      }

      const match = pathname.match(pattern);
      if (!match) continue;

      const number = Number(match[1]);
      if (!Number.isInteger(number) || seen.has(number)) continue;
      seen.add(number);
      numbers.push(number);
    }

    return numbers;
  }

  function issueLabels(issue) {
    return (issue.labels || []).map((label) =>
      typeof label === "string" ? label : label.name || ""
    );
  }

  async function findOpenLinkedPr(issueNumber, config) {
    const timeline = await getIssueTimeline(issueNumber, config);
    const candidates = [];

    for (const event of Array.isArray(timeline) ? timeline : []) {
      if (event.event !== "cross-referenced") continue;
      const sourceIssue = event.source && event.source.issue;
      if (!sourceIssue || !sourceIssue.pull_request) continue;

      const repository = sourceIssue.repository;
      if (
        repository &&
        repository.full_name &&
        repository.full_name !== config.repo
      ) {
        continue;
      }

      const number = sourceIssue.number;
      if (Number.isInteger(number)) candidates.push(number);
    }

    const unique = [...new Set(candidates)];
    for (const prNumber of unique.reverse()) {
      const pr = await githubRequest(
        "GET",
        `/repos/${config.repo}/pulls/${prNumber}`,
        config.token
      );
      if (pr && pr.state === "open") return pr;
    }

    return null;
  }

  async function eligibility(number, config) {
    const issue = await getIssue(number, config);

    if (!issue || issue.pull_request) {
      return { eligible: false, reason: "not an issue" };
    }
    if (issue.state !== "open") {
      return { eligible: false, reason: "closed" };
    }
    if ((config.excludedIssues || []).includes(number)) {
      return { eligible: false, reason: "excluded by configuration" };
    }
    if (Array.isArray(issue.assignees) && issue.assignees.length) {
      return { eligible: false, reason: "already assigned" };
    }

    const blocked = new Set(
      (config.blockedLabels || []).map((label) => label.toLowerCase())
    );
    const matchingBlockedLabel = issueLabels(issue).find((label) =>
      blocked.has(label.toLowerCase())
    );
    if (matchingBlockedLabel) {
      return {
        eligible: false,
        reason: `blocked label: ${matchingBlockedLabel}`,
      };
    }

    const comments = await getIssueComments(number, config);
    if (
      (comments || []).some(
        (comment) =>
          typeof comment.body === "string" &&
          (comment.body.includes(CLAIM_MARKER) ||
            /\bCLAIMED\b/i.test(comment.body))
      )
    ) {
      return { eligible: false, reason: "claim marker/comment already present" };
    }

    const linkedPr = await findOpenLinkedPr(number, config);
    if (linkedPr) {
      return {
        eligible: false,
        reason: `open linked PR #${linkedPr.number}`,
      };
    }

    return { eligible: true, issue };
  }

  function branchName(issueNumber, title) {
    const slug = String(title || "issue")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 48);
    return `agent/${issueNumber}-${slug || "issue"}`;
  }

  function codingPrompt(issue, branch) {
    return [
      `Fix this Search Kernel issue: ${issue.html_url}`,
      "",
      `Repository: ${DEFAULT_REPO}`,
      `Task branch: ${branch}`,
      "",
      "Work autonomously until the implementation is ready for review.",
      "Read the repository-root AGENTS.md first and obey it, including any more-specific AGENTS.md files.",
      "The issue has already been claimed by the issue-autopilot. Do not claim another issue.",
      "Start from current main in a dedicated task worktree/branch. Do not modify another agent's worktree.",
      "For any reported bug, defect, regression, or incorrect behavior, the FIRST code change must be an executable regression test. Run it before production edits and confirm it fails for the reported reason. Only then implement the fix.",
      "Keep the change narrowly scoped to the issue and preserve frozen contracts, Vespa fidelity, and architectural boundaries.",
      "Run the exact verification named by the issue plus the relevant focused checks from AGENTS.md.",
      "Commit and push the task branch, then open a pull request to main that references the issue and contains the concrete verification evidence.",
      "Do not merge the PR, close the issue, or call your own work VERIFIED/CLOSED. Independent review happens after your PR is ready.",
    ].join("\n");
  }

  async function claimIssue(issue, config) {
    // Re-read immediately before mutation. This is the race-closing check.
    const fresh = await eligibility(issue.number, config);
    if (!fresh.eligible) {
      throw new Error(
        `Issue #${issue.number} became ineligible: ${fresh.reason}`
      );
    }

    const branch = branchName(issue.number, issue.title);

    await githubRequest(
      "PATCH",
      `/repos/${config.repo}/issues/${issue.number}`,
      config.token,
      { assignees: [config.assignee] }
    );

    const claimBody = [
      CLAIM_MARKER,
      `CLAIMED by Search Kernel issue autopilot for coding-agent implementation.`,
      "",
      `Task branch: \`${branch}\``,
      "The implementation agent must follow AGENTS.md and deliver the work through a PR.",
      "Final verification/merge remains separate from implementation.",
    ].join("\n");

    try {
      await githubRequest(
        "POST",
        `/repos/${config.repo}/issues/${issue.number}/comments`,
        config.token,
        { body: claimBody }
      );
    } catch (error) {
      // Do not leave a half-claim if the durable claim comment failed.
      await githubRequest(
        "PATCH",
        `/repos/${config.repo}/issues/${issue.number}`,
        config.token,
        { assignees: [] }
      );
      throw error;
    }

    const active = {
      issueNumber: issue.number,
      issueTitle: issue.title,
      issueUrl: issue.html_url,
      branch,
      claimedAt: nowIso(),
      agentDispatchedAt: null,
      prNumber: null,
      prUrl: null,
      reviewRequestedAt: null,
    };

    saveState({
      active,
      pendingAgentPrompt: {
        issueNumber: issue.number,
        prompt: codingPrompt(issue, branch),
        createdAt: nowIso(),
      },
    });

    return active;
  }

  async function pickAndClaim(config) {
    const numbers = visibleIssueNumbers(config);
    if (!numbers.length) {
      throw new Error(
        "No visible issue rows found. Open the repository Issues list with the desired GitHub filters/sort applied."
      );
    }

    const skipped = [];
    for (const number of numbers) {
      const result = await eligibility(number, config);
      if (result.eligible) {
        return claimIssue(result.issue, config);
      }
      skipped.push(`#${number}: ${result.reason}`);
    }

    throw new Error(
      "No eligible visible issues. " + skipped.slice(0, 8).join("; ")
    );
  }

  function getComposer() {
    return (
      document.querySelector("#prompt-textarea") ||
      document.querySelector('textarea[data-id="root"]') ||
      document.querySelector(
        '[contenteditable="true"][data-lexical-editor="true"]'
      ) ||
      document.querySelector('div.ProseMirror[contenteditable="true"]')
    );
  }

  function composerText(composer) {
    if (!composer) return "";
    if (composer instanceof HTMLTextAreaElement) return composer.value || "";
    return composer.innerText || composer.textContent || "";
  }

  function setComposerText(composer, text) {
    if (composer instanceof HTMLTextAreaElement) {
      const setter = Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        "value"
      )?.set;
      if (setter) setter.call(composer, text);
      else composer.value = text;
      composer.dispatchEvent(new Event("input", { bubbles: true }));
      return;
    }

    composer.focus();
    try {
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(composer);
      selection.removeAllRanges();
      selection.addRange(range);
      document.execCommand("insertText", false, text);
    } catch {
      composer.textContent = text;
      composer.dispatchEvent(
        new InputEvent("input", {
          bubbles: true,
          inputType: "insertText",
          data: text,
        })
      );
    }
  }

  function findSendButton() {
    const selectors = [
      'button[data-testid="send-button"]',
      'button[aria-label="Send prompt"]',
      'button[aria-label="Send message"]',
      'button[aria-label="Send"]',
    ];

    for (const selector of selectors) {
      const button = document.querySelector(selector);
      if (button && !button.disabled) return button;
    }

    return null;
  }

  async function submitAgentPrompt(prompt) {
    const composer = getComposer();
    if (!composer) {
      throw new Error("Codex/ChatGPT composer not found");
    }

    if (composerText(composer).trim()) {
      throw new Error("Agent composer is not empty; refusing to overwrite it");
    }

    setComposerText(composer, prompt);
    await new Promise((resolve) => setTimeout(resolve, 500));

    const send = findSendButton();
    if (send) {
      send.click();
    } else {
      composer.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Enter",
          code: "Enter",
          bubbles: true,
          cancelable: true,
        })
      );
    }

    await new Promise((resolve) => setTimeout(resolve, 1200));
    if (composerText(composer).trim()) {
      throw new Error(
        "Coding-agent prompt is still in the composer; submission likely failed"
      );
    }
  }

  async function maybeDispatchAgent(config) {
    if (!isAgentPage(config)) return false;

    const state = loadState();
    if (!state.pendingAgentPrompt || !state.active) return false;

    const pending = state.pendingAgentPrompt;
    if (pending.issueNumber !== state.active.issueNumber) {
      throw new Error("Pending coding-agent prompt does not match active issue");
    }

    await submitAgentPrompt(pending.prompt);

    saveState({
      active: {
        ...state.active,
        agentDispatchedAt: nowIso(),
      },
      pendingAgentPrompt: null,
    });

    saveHealth({
      state: "AGENT_RUNNING",
      message: `Dispatched issue #${state.active.issueNumber} to coding agent.`,
      lastSuccessAt: nowIso(),
      lastError: "",
    });

    return true;
  }

  function openAgentTab(config) {
    GM_openInTab(config.agentUrl, {
      active: true,
      insert: true,
      setParent: true,
    });
  }

  async function requestIndependentReview(pr, active, config) {
    const comments = await githubRequest(
      "GET",
      `/repos/${config.repo}/issues/${pr.number}/comments?per_page=100`,
      config.token
    );

    if (
      (comments || []).some(
        (comment) =>
          typeof comment.body === "string" &&
          (comment.body.includes(REVIEW_MARKER) ||
            /^@codex\s+review\b/im.test(comment.body))
      )
    ) {
      return false;
    }

    const body = [
      "@codex review",
      "",
      REVIEW_MARKER,
      `Independent verification requested by the Search Kernel issue autopilot for issue #${active.issueNumber}.`,
      "Review the implementation against the full issue, repository AGENTS.md, regression-test-first evidence, frozen contracts, architecture invariants, and claimed verification. Focus on substantive correctness/blockers rather than style.",
      "Do not merge or close the issue.",
    ].join("\n");

    await githubRequest(
      "POST",
      `/repos/${config.repo}/issues/${pr.number}/comments`,
      config.token,
      { body }
    );

    return true;
  }

  async function monitorActive(config) {
    const state = loadState();
    const active = state.active;
    if (!active) return;

    const issue = await getIssue(active.issueNumber, config);
    if (!issue || issue.state === "closed") {
      saveState({
        active: null,
        pendingAgentPrompt: null,
        lastCompleted: {
          ...active,
          completedAt: nowIso(),
          reason: "issue closed",
        },
      });
      saveHealth({
        state: "IDLE",
        message: `Issue #${active.issueNumber} closed; queue is free.`,
        lastSuccessAt: nowIso(),
      });
      return;
    }

    const pr = await findOpenLinkedPr(active.issueNumber, config);
    if (!pr) {
      saveHealth({
        state: active.agentDispatchedAt ? "AGENT_RUNNING" : "WAITING_DISPATCH",
        message: active.agentDispatchedAt
          ? `Issue #${active.issueNumber}: waiting for implementation PR.`
          : `Issue #${active.issueNumber}: waiting to dispatch coding agent.`,
        lastSuccessAt: nowIso(),
      });
      return;
    }

    let nextActive = {
      ...active,
      prNumber: pr.number,
      prUrl: pr.html_url,
    };

    if (!pr.draft && !nextActive.reviewRequestedAt) {
      const requested = await requestIndependentReview(pr, nextActive, config);
      if (requested) {
        nextActive = {
          ...nextActive,
          reviewRequestedAt: nowIso(),
        };
      }
    }

    saveState({ active: nextActive });
    saveHealth({
      state: pr.draft ? "PR_DRAFT" : "AWAITING_REVIEW",
      message: pr.draft
        ? `Issue #${active.issueNumber}: PR #${pr.number} is still draft.`
        : `Issue #${active.issueNumber}: PR #${pr.number} is awaiting independent Codex review/final disposition.`,
      lastSuccessAt: nowIso(),
    });
  }

  async function cycle(options = {}) {
    if (cycleInFlight) return;

    const config = loadConfig();
    if (!config.enabled) {
      saveHealth({ state: "OFF", message: "Autopilot disabled." });
      return;
    }
    if (!config.token || !config.repo || !config.assignee || !config.agentUrl) {
      saveHealth({
        state: "NEEDS_CONFIG",
        message: "Configure repository, GitHub token, assignee, and coding-agent URL.",
      });
      return;
    }

    if (!isGitHubIssuesPage() && !isAgentPage(config)) {
      return;
    }

    cycleInFlight = true;
    saveHealth({
      lastAttemptAt: nowIso(),
      lastError: "",
      message: "Checking queue...",
    });

    try {
      await maybeDispatchAgent(config);

      const state = loadState();
      if (state.active) {
        await monitorActive(config);

        const refreshed = loadState();
        if (
          refreshed.active &&
          refreshed.pendingAgentPrompt &&
          !isAgentPage(config)
        ) {
          openAgentTab(config);
          saveHealth({
            state: "WAITING_DISPATCH",
            message: `Opening coding-agent tab for issue #${refreshed.active.issueNumber}.`,
            lastSuccessAt: nowIso(),
          });
        }
      } else if (isGitHubIssuesPage()) {
        const active = await pickAndClaim(config);
        openAgentTab(config);
        saveHealth({
          state: "WAITING_DISPATCH",
          message: `Claimed issue #${active.issueNumber}; opening coding-agent tab.`,
          lastSuccessAt: nowIso(),
        });
      }

      if (options.manual) {
        const health = loadHealth();
        alert(
          [
            "Search Kernel issue autopilot cycle completed.",
            `State: ${health.state}`,
            health.message,
          ].join("\n")
        );
      }
    } catch (error) {
      saveHealth({
        state: "ERROR",
        message: "Autopilot cycle failed.",
        lastErrorAt: nowIso(),
        lastError: conciseError(error),
      });
      console.error("[Search Kernel issue autopilot]", error);

      if (options.manual) {
        alert(`Issue autopilot failed:\n\n${conciseError(error)}`);
      }
    } finally {
      cycleInFlight = false;
      renderStatus();
    }
  }

  function statusLines() {
    const config = loadConfig();
    const state = loadState();
    const health = loadHealth();

    const lines = [
      `Issue autopilot: ${config.enabled ? health.state : "OFF"}`,
      health.message || "",
      `Last success: ${shortTime(health.lastSuccessAt)}`,
    ];

    if (state.active) {
      lines.push(
        `Active: #${state.active.issueNumber} ${state.active.issueTitle || ""}`
      );
      if (state.active.prNumber) {
        lines.push(
          `PR: #${state.active.prNumber} | review: ${
            state.active.reviewRequestedAt ? "requested" : "not yet"
          }`
        );
      } else {
        lines.push(
          `Agent: ${state.active.agentDispatchedAt ? "dispatched" : "pending"}`
        );
      }
    } else {
      lines.push("Active: none");
    }

    if (health.lastError) {
      lines.push(`ERROR: ${health.lastError}`);
    }

    return lines.filter(Boolean);
  }

  function renderStatus() {
    if (!isGitHubIssuesPage()) {
      const existing = document.getElementById(
        "searchkernel-issue-autopilot-status"
      );
      if (existing) existing.remove();
      return;
    }

    let badge = document.getElementById("searchkernel-issue-autopilot-status");
    if (!badge) {
      badge = document.createElement("div");
      badge.id = "searchkernel-issue-autopilot-status";
      badge.style.position = "fixed";
      badge.style.right = "12px";
      badge.style.bottom = "12px";
      badge.style.zIndex = "2147483647";
      badge.style.padding = "8px 10px";
      badge.style.borderRadius = "8px";
      badge.style.font =
        "12px/1.35 ui-monospace, SFMono-Regular, Menlo, monospace";
      badge.style.color = "white";
      badge.style.whiteSpace = "pre";
      badge.style.maxWidth = "560px";
      badge.style.boxShadow = "0 2px 8px rgba(0,0,0,.25)";
      document.documentElement.appendChild(badge);
    }

    const health = loadHealth();
    badge.style.background =
      health.state === "ERROR"
        ? "#b42318"
        : health.state === "AWAITING_REVIEW"
          ? "#6941c6"
          : health.state === "AGENT_RUNNING" ||
              health.state === "WAITING_DISPATCH"
            ? "#175cd3"
            : loadConfig().enabled
              ? "#067647"
              : "#555";
    badge.textContent = statusLines().join("\n");
  }

  function configure() {
    const old = loadConfig();

    const token =
      prompt(
        "Fine-grained GitHub token with Issues read/write and Pull requests read/write for superkelvint/searchkernel:",
        old.token || ""
      ) ??
      old.token ??
      "";

    const assignee =
      prompt("GitHub login used to claim issues:", old.assignee) ||
      old.assignee ||
      DEFAULT_ASSIGNEE;

    const agentUrl =
      prompt(
        "Coding-agent page URL (the script will open this and submit the issue prompt):",
        old.agentUrl || DEFAULT_AGENT_URL
      ) ||
      old.agentUrl ||
      DEFAULT_AGENT_URL;

    const excludedRaw =
      prompt(
        "Comma-separated issue numbers to never pick:",
        (old.excludedIssues || []).join(",")
      ) || "";

    const blockedRaw =
      prompt(
        "Comma-separated labels that make an issue ineligible:",
        (old.blockedLabels || []).join(",")
      ) || "";

    saveConfig({
      ...old,
      token: token.trim(),
      assignee: assignee.trim(),
      agentUrl: normalizeUrl(agentUrl.trim()) || agentUrl.trim(),
      excludedIssues: excludedRaw
        .split(",")
        .map((value) => Number(value.trim()))
        .filter(Number.isInteger),
      blockedLabels: blockedRaw
        .split(",")
        .map((value) => value.trim())
        .filter(Boolean),
      enabled: true,
    });

    saveHealth({
      state: "IDLE",
      message: "Configured; autopilot enabled.",
      lastError: "",
    });

    if (isGitHubIssuesPage()) {
      cycle({ manual: false });
    }
  }

  function toggleEnabled() {
    const config = loadConfig();
    config.enabled = !config.enabled;
    saveConfig(config);
    saveHealth({
      state: config.enabled ? "IDLE" : "OFF",
      message: config.enabled ? "Autopilot enabled." : "Autopilot disabled.",
      lastError: "",
    });

    if (config.enabled) cycle({ manual: false });
  }

  function clearActive() {
    const state = loadState();
    if (!state.active) {
      alert("There is no active autopilot issue.");
      return;
    }

    if (
      !confirm(
        `Forget active issue #${state.active.issueNumber}?\n\nThis does NOT unassign the GitHub issue, delete the claim comment, close a PR, or undo any coding-agent work.`
      )
    ) {
      return;
    }

    saveState({
      active: null,
      pendingAgentPrompt: null,
      lastCompleted: {
        ...state.active,
        completedAt: nowIso(),
        reason: "operator cleared local autopilot state",
      },
    });
    saveHealth({
      state: "IDLE",
      message: "Local active state cleared.",
      lastError: "",
    });
  }

  function showDiagnostics() {
    alert(
      JSON.stringify(
        {
          version: "0.1.0",
          config: {
            ...loadConfig(),
            token: loadConfig().token ? "<configured>" : "",
          },
          state: loadState(),
          health: loadHealth(),
          currentUrl: location.href,
          visibleIssueNumbers: isGitHubIssuesPage()
            ? visibleIssueNumbers(loadConfig())
            : [],
        },
        null,
        2
      )
    );
  }

  GM_registerMenuCommand("Configure Search Kernel issue autopilot", configure);
  GM_registerMenuCommand("Run issue autopilot now", () =>
    cycle({ manual: true })
  );
  GM_registerMenuCommand("Toggle issue autopilot", toggleEnabled);
  GM_registerMenuCommand("Clear local active issue", clearActive);
  GM_registerMenuCommand("Show issue autopilot diagnostics", showDiagnostics);

  renderStatus();

  setTimeout(() => {
    cycle({ manual: false });
  }, 2500);

  setInterval(() => {
    cycle({ manual: false });
  }, POLL_MS);

  let lastUrl = location.href;
  setInterval(() => {
    if (location.href !== lastUrl) {
      lastUrl = location.href;
      renderStatus();
      cycle({ manual: false });
    }
  }, 1000);
})();
