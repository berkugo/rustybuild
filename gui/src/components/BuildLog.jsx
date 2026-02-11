import { useEffect, useRef, useState, useMemo } from "react";

export default function BuildLog({ lines, success, buildRunning = false, onCancelBuild, className = "" }) {
  const logEndRef = useRef(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const [filter, setFilter] = useState("all");
  const [expandedTargets, setExpandedTargets] = useState(new Set());

  const scrollContainerRef = useRef(null);
  useEffect(() => {
    if (!autoScroll || !scrollContainerRef.current || !logEndRef.current) return;
    const el = scrollContainerRef.current;
    const targetScroll = el.scrollHeight - el.clientHeight;
    if (targetScroll <= 0) return;
    const start = el.scrollTop;
    const startTime = performance.now();
    const duration = 3000;
    const tick = (now) => {
      const t = Math.min((now - startTime) / duration, 1);
      const ease = 1 - (1 - t) * (1 - t);
      el.scrollTop = start + (targetScroll - start) * ease;
      if (t < 1) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }, [lines, autoScroll]);

  // Parse build output into structured data (supports [TARGET:name] prefix from Ninja-style builder)
  const buildData = useMemo(() => {
    let targetMap = new Map(); // name -> target (for [TARGET:name] lines)
    let targets = [];
    let currentTarget = null;
    let totalTargets = 0;
    let successfulTargets = 0;
    let failedTargets = 0;
    let totalFromStart = 0;
    let currentLevel = null;

    const applyMessageToTarget = (t, rest) => {
      if (rest.includes("[OK]") || rest.includes("Success") || rest.includes("successfully")) {
        t.status = "success";
        t.messages.push({ type: "success", text: rest });
      } else if (rest.includes("[ERROR]") || (rest.toLowerCase().includes("failed") && !rest.includes("up-to-date"))) {
        t.status = "error";
        t.errorCount++;
        t.messages.push({ type: "error", text: rest });
      } else if (rest.includes("[SKIP]")) {
        if (rest.includes("Linking") && rest.includes("up-to-date")) {
          t.status = "skipped";
        }
        t.messages.push({ type: "info", text: rest });
      } else if (rest.includes("[COMPILE]")) {
        t.compileCount++;
        if (t.compileCount <= 1 || rest.includes("error") || rest.includes("ERROR")) {
          t.messages.push({ type: "compile", text: rest });
        }
      } else if (rest.includes("[LINK]") || rest.includes("[ARCHIVE]")) {
        t.messages.push({ type: "link", text: rest });
      } else if (rest.trim() && (rest.includes("Command:") || rest.includes("stderr:") || rest.includes("fatal error") || rest.includes("error:"))) {
        t.messages.push({ type: "detail", text: rest });
      } else if (rest.trim()) {
        // Continuation lines (e.g. compiler stderr lines sent after [ERROR])
        t.messages.push({ type: "detail", text: rest });
      }
    };

    for (const line of lines) {
      if (line.startsWith("__OXIMAKE_TOTAL__\t")) {
        const n = parseInt(line.split("\t")[1], 10);
        if (!isNaN(n)) totalFromStart = n;
        // New build started: reset targets and summary so progress starts at 0/N
        targetMap = new Map();
        targets = [];
        currentTarget = null;
        totalTargets = 0;
        successfulTargets = 0;
        failedTargets = 0;
        continue;
      }
      if (line.startsWith("__OXIMAKE_FINISH__")) continue;

      // Ninja-style: [TARGET:name] message
      if (line.startsWith("[TARGET:") && line.includes("] ")) {
        const idx = line.indexOf("] ");
        const name = line.slice(8, idx);
        const rest = line.slice(idx + 2);
        let t = targetMap.get(name);
        if (!t) {
          t = { name, type: "unknown", status: "building", messages: [], compileCount: 0, errorCount: 0, level: null };
          targetMap.set(name, t);
        }
        if (rest.includes("===") && rest.includes("Building target")) {
          t.type = rest.includes("Executable") ? "executable" : rest.includes("StaticLib") ? "static_lib" : rest.includes("SharedLib") ? "shared_lib" : "unknown";
        }
        applyMessageToTarget(t, rest);
        continue;
      }

      // Summary lines
      if (line.includes("Total:") && line.includes("targets")) {
        const match = line.match(/Total:\s*(\d+)\s*targets?/);
        if (match) totalTargets = parseInt(match[1]);
        const successMatch = line.match(/(\d+)\s+successful/);
        if (successMatch) successfulTargets = parseInt(successMatch[1]);
        const failedMatch = line.match(/(\d+)\s+failed/);
        if (failedMatch) failedTargets = parseInt(failedMatch[1]);
      }
      if (line.includes("---") && line.includes("targets") && line.includes("successful")) {
        const match = line.match(/(\d+)\s+targets?/);
        if (match) totalTargets = parseInt(match[1]);
        const successMatch = line.match(/(\d+)\s+successful/);
        if (successMatch) successfulTargets = parseInt(successMatch[1]);
        const failedMatch = line.match(/(\d+)\s+failed/);
        if (failedMatch) failedTargets = parseInt(failedMatch[1]);
      }

      if (line.includes("Level") && line.includes("target(s)")) {
        const match = line.match(/Level\s+(\d+)/);
        if (match) currentLevel = parseInt(match[1]);
      }

      // Legacy: === Building target 'name' (no prefix)
      if (line.startsWith("===") && line.includes("Building target")) {
        const match = line.match(/target\s+['"]([^'"]+)['"]/);
        if (match) {
          const name = match[1];
          if (currentTarget && currentTarget.name !== name) {
            targets.push(currentTarget);
          }
          if (!currentTarget || currentTarget.name !== name) {
            currentTarget = {
              name,
              type: line.includes("Executable") ? "executable" : line.includes("StaticLib") ? "static_lib" : line.includes("SharedLib") ? "shared_lib" : "unknown",
              status: "building",
              messages: [],
              compileCount: 0,
              errorCount: 0,
              level: currentLevel,
            };
          }
        }
      }

      if (currentTarget && !line.startsWith("[TARGET:")) {
        applyMessageToTarget(currentTarget, line);
      }
    }

    if (currentTarget) {
      targets.push(currentTarget);
    }
    // Prefer Ninja-style targets from map when present
    const fromMap = Array.from(targetMap.values());
    const finalTargets = fromMap.length > 0 ? fromMap : targets;

    // Deduplicate by target name: keep first occurrence (full messages), drop later duplicates
    const seen = new Set();
    const deduped = finalTargets.filter((t) => {
      if (seen.has(t.name)) return false;
      seen.add(t.name);
      return true;
    });

    // Fix status for targets that don't have explicit [OK] or [ERROR] messages
    // If a target has messages but no explicit status, check if it completed successfully
    // Also use build summary to infer status if all targets are successful
    const allTargetsSuccessful = totalTargets > 0 && failedTargets === 0 && successfulTargets === totalTargets;
    
    for (const target of deduped) {
      // Only "skipped" when the *link* was skipped (whole target up-to-date). If any file was recompiled we get [LINK] Creating / [OK], so no link-skip.
      const linkWasSkipped = target.messages.some(m =>
        m.text.includes("[SKIP]") && m.text.includes("Linking") && m.text.includes("up-to-date")
      );
      if (linkWasSkipped) {
        target.status = "skipped";
        continue;
      }
      if (target.status === "building" || target.status === "error") {
        const hasError = target.messages.some(m =>
          m.type === "error" || m.text.includes("[ERROR]") ||
          (m.text.toLowerCase().includes("failed") && !m.text.includes("up-to-date"))
        );
        const hasSuccess = target.messages.some(m =>
          m.type === "success" || m.text.includes("[OK]") ||
          m.text.includes("Success") || m.text.includes("successfully")
        );
        if (allTargetsSuccessful && !hasError) {
          target.status = "success";
        } else if (target.messages.length > 0 && !hasError) {
          target.status = "success";
        } else if (hasSuccess && !hasError) {
          target.status = "success";
        } else if (hasError) {
          target.status = "error";
        }
      }
    }

    // During build: completed = targets that are done (not "building")
    const completedCount = deduped.filter(t => t.status !== "building").length;
    const totalSeen = deduped.length;

    return {
      targets: deduped,
      summary: {
        total: totalTargets,
        successful: successfulTargets,
        failed: failedTargets,
      },
      totalFromStart,
      totalSeen,
      completedCount,
    };
  }, [lines]);

  // Progress: when totalFromStart is set (build sent __OXIMAKE_TOTAL__), use it and completedCount so we start at 0/N
  const totalForProgress = buildData.totalFromStart > 0
    ? buildData.totalFromStart
    : (buildData.summary.total > 0 ? buildData.summary.total : Math.max(buildData.targets.length, 1));
  const hasSummary = buildData.summary.total > 0;
  const doneForProgress = buildData.totalFromStart > 0
    ? buildData.completedCount
    : (hasSummary ? buildData.summary.successful + buildData.summary.failed : buildData.completedCount);
  const progress = Math.round((doneForProgress / totalForProgress) * 100);
  const showProgressBar = lines.length > 0;

  const filteredTargets = useMemo(() => {
    if (filter === "all") return buildData.targets;
    if (filter === "error") return buildData.targets.filter(t => t.status === "error");
    if (filter === "success") return buildData.targets.filter(t => t.status === "success");
    if (filter === "warning") return buildData.targets.filter(t => t.status === "skipped");
    return buildData.targets;
  }, [buildData.targets, filter]);

  const toggleTarget = (targetName) => {
    const newExpanded = new Set(expandedTargets);
    if (newExpanded.has(targetName)) {
      newExpanded.delete(targetName);
    } else {
      newExpanded.add(targetName);
    }
    setExpandedTargets(newExpanded);
  };

  const getTargetIcon = (target, effectiveStatus) => {
    const baseClass = "w-5 h-5 flex-shrink-0";
    if (target.status === "building") {
      return (
        <svg className={baseClass + " text-blue-400 animate-spin"} viewBox="0 0 24 24">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" fill="none" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
        </svg>
      );
    }
    switch (effectiveStatus) {
      case "success":
        return (
          <svg className={baseClass + " text-green-400"} fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
          </svg>
        );
      case "error":
        return (
          <svg className={baseClass + " text-red-400"} fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        );
      case "skipped":
        return (
          <span className="text-[10px] font-semibold uppercase tracking-wider text-teal-400 px-1.5 py-0.5 rounded bg-teal-500/20 border border-teal-500/40">
            SKIPPED
          </span>
        );
      default:
        return (
          <svg className={baseClass + " text-green-400"} fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
          </svg>
        );
    }
  };

  const getTypeBadge = (type) => {
    const colors = {
      executable: "bg-red-500/15 text-red-400 border-red-500/40",
      static_lib: "bg-teal-500/15 text-teal-400 border-teal-500/40",
      shared_lib: "bg-blue-500/15 text-blue-400 border-blue-500/40",
    };
    const labels = {
      executable: "Exe",
      static_lib: "Static",
      shared_lib: "Shared",
    };
    return (
      <span className={`inline-flex items-center justify-center min-w-[3.5rem] px-1.5 py-0.5 rounded text-[10px] font-medium border ${colors[type] || colors.static_lib}`}>
        {labels[type] || type.replace(/_/g, " ")}
      </span>
    );
  };

  const shortenMessage = (text) => {
    // Shorten compile messages
    if (text.includes("[COMPILE]")) {
      const match = text.match(/\[COMPILE\]\s+(.+?)\s+→\s+(.+)/);
      if (match) {
        const file = match[1].split('/').pop();
        return `[COMPILE] ${file}`;
      }
    }
    // Shorten link messages
    if (text.includes("[LINK]")) {
      const match = text.match(/\[LINK\]\s+(.+)/);
      if (match) {
        return `[LINK] ${match[1]}`;
      }
    }
    // Keep error messages full
    if (text.includes("error") || text.includes("ERROR") || text.includes("fatal")) {
      return text;
    }
    // Truncate long messages
    if (text.length > 120) {
      return text.substring(0, 120) + "...";
    }
    return text;
  };

  return (
    <div className={`h-full min-h-0 flex flex-col bg-gray-950 overflow-hidden ${className}`.trim()}>
      {/* Compact header: title, status, progress, filters */}
      <div className="flex-shrink-0 px-4 py-2.5 border-b border-gray-800 bg-gray-900/80">
        <div className="flex flex-wrap items-center gap-3">
          <h2 className="text-sm font-semibold text-white">Build Output</h2>
          {success !== null && (
            <span className={`px-2 py-0.5 rounded text-xs font-medium ${
              success ? "bg-green-500/20 text-green-400" : "bg-red-500/20 text-red-400"
            }`}>
              {success ? "OK" : "Failed"}
            </span>
          )}
          {showProgressBar && (
            <>
              <span className="text-xs text-gray-400">
                {buildData.summary.failed > 0
                  ? `${buildData.summary.successful} OK, ${buildData.summary.failed} failed`
                  : hasSummary
                    ? `${buildData.summary.successful}/${buildData.summary.total}`
                    : `${doneForProgress}/${totalForProgress}`}
              </span>
              <span className="text-xs font-semibold text-gray-300 tabular-nums">{progress}%</span>
              <div className="w-24 h-1.5 bg-gray-800 rounded-full overflow-hidden">
                <div
                  className={`h-full transition-all duration-300 rounded-full ${
                    success ? "bg-green-500" : buildData.summary.failed > 0 ? "bg-red-500" : "bg-blue-500"
                  }`}
                  style={{ width: `${progress}%` }}
                />
              </div>
              {buildRunning && onCancelBuild && (
                <button
                  type="button"
                  onClick={onCancelBuild}
                  className="flex items-center justify-center w-7 h-7 rounded-md bg-red-500/20 hover:bg-red-500/30 border border-red-500/40 text-red-400 hover:text-red-300 transition-colors"
                  title="Stop build"
                >
                  <svg className="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                    <rect x="6" y="6" width="12" height="12" rx="1" />
                  </svg>
                </button>
              )}
            </>
          )}
          <div className="flex gap-1 ml-auto">
            {["all", "error", "success"].map((f) => (
              <button
                key={f}
                onClick={() => setFilter(f)}
                className={`px-2 py-1 rounded text-xs font-medium ${
                  filter === f
                    ? "bg-gray-700 text-white"
                    : "text-gray-400 hover:text-white hover:bg-gray-800"
                }`}
              >
                {f === "all" ? "All" : f === "error" ? "Err" : "OK"}
              </button>
            ))}
            <label className="flex items-center gap-1.5 pl-2 text-xs text-gray-400 cursor-pointer select-none">
              <input
                type="checkbox"
                checked={autoScroll}
                onChange={(e) => setAutoScroll(e.target.checked)}
                className="w-3 h-3 rounded border-gray-600 bg-gray-800"
              />
              Auto
            </label>
          </div>
        </div>
      </div>

      {/* Scrollable targets list - min-h-0 is required for flex child to scroll */}
      <div ref={scrollContainerRef} className="build-log-scroll flex-1 min-h-0 overflow-y-auto overflow-x-hidden">
        {filteredTargets.length === 0 ? (
          lines.length === 0 ? (
            <div className="h-full flex items-center justify-center">
              <div className="text-center space-y-3">
                <svg className="w-16 h-16 mx-auto text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                </svg>
                <p className="text-gray-400">No build run yet.</p>
              </div>
            </div>
          ) : buildData.targets.length === 0 ? (
            <div className="p-3 space-y-1">
              <div className="rounded-lg border border-slate-700/50 bg-slate-800/30 px-3 py-2 mb-2">
                <p className="text-xs font-medium text-slate-400 mb-2">Output</p>
                {lines.filter((l) => !l.startsWith("__OXIMAKE_")).map((line, i) => (
                  <div key={i} className={`text-xs font-mono py-0.5 ${line.includes("[ERROR]") ? "text-red-400" : line.includes("[CLEAN]") ? "text-teal-400/90" : "text-slate-300"}`}>
                    {line}
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div className="h-full flex items-center justify-center">
              <p className="text-gray-400">No targets match the selected filter.</p>
            </div>
          )
        ) : (
          <div className="p-3 space-y-2 w-full min-w-0">
            {filteredTargets.map((target, idx) => {
              const isExpanded = expandedTargets.has(target.name);
              const hasDetails = target.messages.length > 1 || target.compileCount > 1;
              
              // Determine effective status for styling (handle "building" status that should be success)
              // Use build summary to infer status: if all targets are successful, mark as success
              const allSuccessful = buildData.summary.total > 0 && 
                                   buildData.summary.failed === 0 && 
                                   buildData.summary.successful === buildData.summary.total;
              
              const hasError = target.messages.some(m => 
                m.type === "error" || m.text.includes("[ERROR]") || 
                (m.text.toLowerCase().includes("failed") && !m.text.includes("up-to-date"))
              );
              
              let effectiveStatus = target.status;
              
              // If build summary says all successful and this target has no errors, treat as success
              if (allSuccessful && !hasError && (target.status === "building" || target.status === "error")) {
                effectiveStatus = "success";
              } else if (target.status === "building" && target.messages.length > 0 && !hasError) {
                // If building but has messages and no errors, assume success
                effectiveStatus = "success";
              }
              
              return (
                <div
                  key={idx}
                  className={`w-full rounded-lg border transition-all ${
                    effectiveStatus === "error"
                      ? "bg-red-500/5 border-red-500/30"
                      : effectiveStatus === "success"
                      ? "bg-green-500/5 border-green-500/30"
                      : effectiveStatus === "skipped"
                      ? "bg-teal-500/5 border-teal-500/30"
                      : "bg-gray-800/30 border-gray-700/50"
                  }`}
                >
                  <button
                    onClick={() => hasDetails && toggleTarget(target.name)}
                    className={`w-full flex gap-3 p-2.5 text-left items-center ${
                      hasDetails ? "hover:bg-gray-800/40 cursor-pointer" : "cursor-default"
                    }`}
                  >
                    <div className="flex-shrink-0 flex items-center justify-center min-w-7 h-7 px-1 rounded bg-gray-800/80 border border-gray-700/50">
                      {getTargetIcon(target, effectiveStatus)}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="font-medium text-white text-sm">{target.name}</span>
                        {target.compileCount > 0 && (
                          <span className="text-[10px] text-gray-500">{target.compileCount} file(s)</span>
                        )}
                      </div>
                      {target.messages.length > 0 && !isExpanded && (
                        <div className="text-[11px] text-gray-500 truncate font-mono mt-0.5">
                          {shortenMessage(target.messages[target.messages.length - 1].text)}
                        </div>
                      )}
                    </div>
                    <div className="flex-shrink-0">{getTypeBadge(target.type)}</div>
                    {hasDetails && (
                      <svg
                        className={`w-4 h-4 text-gray-500 flex-shrink-0 transition-transform ${isExpanded ? "rotate-180" : ""}`}
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                      </svg>
                    )}
                  </button>
                  {isExpanded && hasDetails && (
                    <div className="px-3 pb-3 pt-1 space-y-1 border-t border-gray-700/30">
                      {target.messages.map((msg, msgIdx) => (
                        <div
                          key={msgIdx}
                          className={`text-[11px] font-mono pl-3 pr-2 py-1.5 rounded break-words ${
                            msg.type === "error" ? "bg-red-500/10 text-red-300" :
                            msg.type === "success" ? "bg-green-500/10 text-green-300" :
                            msg.type === "compile" ? "bg-blue-500/10 text-blue-300" :
                            msg.type === "link" ? "bg-purple-500/10 text-purple-300" :
                            "bg-gray-800/50 text-gray-300"
                          }`}
                        >
                          <pre className="whitespace-pre-wrap leading-snug">{msg.text}</pre>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
            <div ref={logEndRef} />
          </div>
        )}
      </div>
    </div>
  );
}
