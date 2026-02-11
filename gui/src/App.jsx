import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import DependencyGraph from "./components/DependencyGraph";
import BuildTomlEditor from "./components/BuildTomlEditor";
import BuildLog from "./components/BuildLog";
import FileTree from "./components/FileTree";
import NewProjectModal from "./components/NewProjectModal";

function App() {
  const [configPath, setConfigPath] = useState(null);
  const [projectInfo, setProjectInfo] = useState(null);
  const [tomlContent, setTomlContent] = useState("");
  const [activeTab, setActiveTab] = useState("graph");
  const [buildLog, setBuildLog] = useState([]);
  const [buildRunning, setBuildRunning] = useState(false);
  const [parseError, setParseError] = useState(null);
  const [buildSuccess, setBuildSuccess] = useState(null);
  const [projectRoot, setProjectRoot] = useState(null);
  const [buildTomlFiles, setBuildTomlFiles] = useState([]);
  const [showNewProjectModal, setShowNewProjectModal] = useState(false);
  const [buildJobs, setBuildJobs] = useState(0); // 0 = auto (used as default when opening modal)
  const [maxJobs, setMaxJobs] = useState(8);
  const [showBuildModal, setShowBuildModal] = useState(false);
  const [buildModalClean, setBuildModalClean] = useState(false); // true = Clean & Build
  const [buildIgnoreErrors, setBuildIgnoreErrors] = useState(false);

  useEffect(() => {
    invoke("get_max_jobs").then((n) => setMaxJobs(n)).catch(() => setMaxJobs(8));
  }, []);

  const loadFile = useCallback(async (path) => {
    setConfigPath(path);
    setParseError(null);
    const dir = path.includes("/") ? path.split("/").slice(0, -1).join("/") : "";
    setProjectRoot((prev) => prev || dir);
    try {
      const content = await invoke("read_file", { path });
      setTomlContent(content);
    } catch (e) {
      setParseError(String(e));
      return;
    }
    try {
      const info = await invoke("parse_build_toml", { path });
      setProjectInfo(info);
    } catch (e) {
      setParseError(String(e));
      setProjectInfo(null);
    }
    if (dir) {
      invoke("find_build_toml_files", { rootPath: dir })
        .then((files) => {
          setBuildTomlFiles((prev) => {
            if (prev.length > 0) return prev;
            return Array.isArray(files) ? files : [];
          });
        })
        .catch(() => {});
    }
  }, []);

  const handleFilePicker = useCallback(async () => {
    try {
      const selected = await invoke("open_file_dialog");
      if (selected) {
        await loadFile(selected);
      }
    } catch (e) {
      console.error("File picker error:", e);
      setParseError(`Error selecting file: ${e}`);
    }
  }, [loadFile]);

  const handleCreateNew = useCallback(() => {
    setShowNewProjectModal(true);
  }, []);

  const handleCreateProject = useCallback(async (projectData) => {
    try {
      const result = await invoke("init_project", {
        projectName: projectData.name,
        cppVersion: projectData.cppVersion,
        projectType: projectData.projectType,
      });
      
      setTomlContent(result.toml_content);
      setConfigPath(result.config_path);
      setProjectRoot(result.project_root);
      setBuildTomlFiles([]);
      setProjectInfo(null);
      setParseError(null);
      setActiveTab("files");
      
      // Try to parse the created project
      try {
        const info = await invoke("parse_build_toml", { path: result.config_path });
        setProjectInfo(info);
      } catch (parseErr) {
        console.error("Parse error:", parseErr);
      }
    } catch (e) {
      setParseError(`Failed to create project: ${e}`);
    }
  }, []);

  const handleConvertCmake = useCallback(async () => {
    try {
      const selected = await invoke("open_cmake_dialog");
      if (!selected) return;

      setBuildRunning(true);
      setParseError(null);
      
      const result = await invoke("convert_cmake_to_toml", { cmakePath: selected });
      
      const cmakePath = selected;
      const baseDir = cmakePath.substring(0, cmakePath.lastIndexOf("/"));
      const suggestedPath = `${baseDir}/build.toml`;
      
      setTomlContent(result.toml_content);
      setConfigPath(suggestedPath);
      setProjectRoot(result.project_root);
      setBuildTomlFiles(result.build_toml_files || []);
      setActiveTab("files");
      
      try {
        await invoke("write_file", { path: suggestedPath, content: result.toml_content || result.tomlContent });
        const info = await invoke("parse_build_toml", { path: suggestedPath });
        setProjectInfo(info);
      } catch (parseErr) {
        console.error("Parse error:", parseErr);
        setParseError(`Parsing error: ${parseErr}. The file was converted but may need manual editing.`);
        setProjectInfo(null);
      }
    } catch (e) {
      setParseError(`CMake conversion error: ${e}`);
    } finally {
      setBuildRunning(false);
    }
  }, []);

  const handleFileSelect = useCallback(async (filePath, isBuildToml) => {
    if (isBuildToml) await loadFile(filePath);
    // Stay on Project Files tab; editor shows when configPath is set
  }, [loadFile]);

  const handleSaveToml = useCallback(async () => {
    if (!configPath) return;
    try {
      await invoke("write_file", {
        path: configPath,
        content: tomlContent,
      });
      await loadFile(configPath);
    } catch (e) {
      setParseError(String(e));
    }
  }, [configPath, tomlContent, loadFile]);

  // Build event unlisteners (thread-safe: backend emits on channel, frontend updates state on main thread)
  const buildUnlistenRef = useRef([]);
  const listenersReadyRef = useRef(false);
  useEffect(() => {
    listenersReadyRef.current = false;
    let done = 0;
    const maybeReady = () => {
      done += 1;
      if (done === 2) listenersReadyRef.current = true;
    };
    listen("build-output", (event) => {
      const raw = event?.payload ?? event;
      const line = typeof raw === "string" ? raw : (raw?.payload != null ? String(raw.payload) : String(raw));
      setBuildLog((prev) => {
        if (prev.length > 0 && prev[prev.length - 1] === line) return prev;
        return [...prev, line];
      });
    }, { target: "main" }).then((fn) => {
      buildUnlistenRef.current.push(fn);
      maybeReady();
    }).catch((err) => {
      console.error("build-output listen failed:", err);
    });
    listen("build-finished", (event) => {
      const p = (event?.payload ?? event) || {};
      setBuildSuccess(p.success ?? false);
      setBuildRunning(false);
    }, { target: "main" }).then((fn) => {
      buildUnlistenRef.current.push(fn);
      maybeReady();
    }).catch((err) => {
      console.error("build-finished listen failed:", err);
    });
    return () => {
      buildUnlistenRef.current.forEach((fn) => typeof fn === "function" && fn());
      buildUnlistenRef.current = [];
      listenersReadyRef.current = false;
    };
  }, []);

  const handleBuild = useCallback(async (clean, jobs, ignoreErrors) => {
    if (!configPath) return;
    setShowBuildModal(false);
    setBuildRunning(true);
    setBuildLog([]);
    setBuildSuccess(null);
    setActiveTab("build");
    await new Promise((r) => setTimeout(r, 200));
    try {
      await invoke("run_build_async", {
        configPath,
        targets: null,
        clean,
        jobs: jobs === 0 ? null : jobs,
        ignoreErrors: !!ignoreErrors,
      });
    } catch (e) {
      setBuildLog((prev) => [...prev, `[ERROR] ${e}`]);
      setBuildSuccess(false);
      setBuildRunning(false);
    }
  }, [configPath]);

  const handleCancelBuild = useCallback(() => {
    invoke("cancel_build").catch(() => {});
  }, []);

  const handleClean = useCallback(async () => {
    if (!configPath) return;
    setBuildRunning(true);
    setBuildLog([]);
    setBuildSuccess(null);
    setActiveTab("build");
    await new Promise((r) => setTimeout(r, 200));
    try {
      await invoke("run_clean_async", { configPath });
    } catch (e) {
      setBuildLog((prev) => [...prev, `[ERROR] ${e}`]);
      setBuildSuccess(false);
      setBuildRunning(false);
    }
  }, [configPath]);

  return (
    <div className="h-screen flex flex-col bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950 overflow-hidden">
      {/* Modern Header */}
      <header className="relative z-50 flex items-center justify-between px-8 py-5 bg-slate-900/80 backdrop-blur-xl border-b border-slate-800/50 shadow-2xl">
        <div className="flex items-center gap-6">
          {/* Logo — wordmark only */}
          <h1 className="text-2xl font-extrabold tracking-tight select-none">
            <span className="bg-gradient-to-r from-emerald-400 via-teal-400 to-cyan-500 bg-clip-text text-transparent drop-shadow-sm">
              ng
            </span>
            <span className="bg-gradient-to-r from-slate-200 to-slate-400 bg-clip-text text-transparent">
              make
            </span>
          </h1>

          {/* Divider */}
          <div className="h-8 w-px bg-gradient-to-b from-transparent via-slate-700 to-transparent" />

          {/* Current target + Import — single row, aligned */}
          <div className="flex items-center gap-2 rounded-xl bg-slate-800/40 border border-slate-700/50 px-3 py-1.5">
            <span className="text-[10px] font-medium text-slate-500 uppercase tracking-wider whitespace-nowrap">Current target</span>
            {configPath ? (
              <>
                <div
                  className="px-2.5 py-1 rounded-md bg-slate-800/80 border border-slate-600/50 text-slate-200 text-sm font-mono truncate max-w-[220px]"
                  title={configPath}
                >
                  {projectRoot && configPath.startsWith(projectRoot + "/")
                    ? configPath.slice(projectRoot.length + 1)
                    : configPath.split("/").slice(-2).join("/") || "build.toml"}
                </div>
                <button
                  onClick={handleFilePicker}
                  className="flex-shrink-0 p-1.5 rounded-md hover:bg-slate-700/80 border border-transparent hover:border-slate-600 text-slate-400 hover:text-white transition-all"
                  title="Import another build.toml"
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                  </svg>
                </button>
              </>
            ) : (
              <button
                onClick={handleFilePicker}
                className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-slate-700/50 hover:bg-slate-700 border border-slate-600/50 text-slate-300 hover:text-white text-sm font-medium transition-all"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                </svg>
                Import
              </button>
            )}
          </div>
        </div>

        {/* Build Actions + Parallel jobs below */}
        {configPath && (
          <div className="flex flex-col items-end gap-2">
            <div className="flex items-center gap-3">
              <button
                onClick={() => { setBuildModalClean(false); setShowBuildModal(true); }}
                disabled={buildRunning}
                className="group relative px-5 py-2.5 rounded-lg bg-gradient-to-r from-emerald-500 via-teal-500 to-cyan-600 hover:from-emerald-600 hover:via-teal-600 hover:to-cyan-700 text-white font-semibold text-sm transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2 shadow-lg shadow-emerald-500/20 hover:shadow-xl hover:shadow-teal-500/30 hover:scale-105 disabled:hover:scale-100 overflow-hidden"
              >
                <div className="absolute inset-0 bg-gradient-to-r from-white/0 via-white/20 to-white/0 translate-x-[-100%] group-hover:translate-x-[100%] transition-transform duration-700" />
                {buildRunning ? (
                  <>
                    <svg className="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                    </svg>
                    <span className="relative">Building...</span>
                  </>
                ) : (
                  <>
                    <svg className="w-4 h-4 relative" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <span className="relative">Build</span>
                  </>
                )}
              </button>
              <button
                onClick={handleClean}
                disabled={buildRunning}
                className="px-4 py-2.5 rounded-lg bg-slate-800/50 hover:bg-slate-800 border border-slate-700/50 hover:border-slate-600 text-slate-200 hover:text-white font-medium text-sm transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
                Clean
              </button>
              <button
                onClick={() => { setBuildModalClean(true); setShowBuildModal(true); }}
                disabled={buildRunning}
                className="px-4 py-2.5 rounded-lg bg-slate-800/50 hover:bg-slate-800 border border-slate-700/50 hover:border-slate-600 text-slate-200 hover:text-white font-medium text-sm transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
                Clean & Build
              </button>
            </div>
          </div>
        )}
      </header>

      {/* Build options modal (Build / Clean & Build) */}
      {showBuildModal && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm" onClick={() => setShowBuildModal(false)}>
          <div className="bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl w-full max-w-md overflow-hidden" onClick={(e) => e.stopPropagation()}>
            <div className="px-6 py-4 border-b border-slate-700/50">
              <h2 className="text-lg font-semibold text-white">
                {buildModalClean ? "Clean & Build" : "Build"} options
              </h2>
            </div>
            <div className="px-6 py-5 space-y-5">
              <div>
                <label className="block text-sm font-medium text-slate-400 mb-2">Parallel jobs</label>
                <select
                  value={buildJobs}
                  onChange={(e) => setBuildJobs(Number(e.target.value))}
                  className="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-slate-200 text-sm focus:ring-1 focus:ring-emerald-500 focus:border-emerald-500"
                >
                  <option value={0}>Auto</option>
                  {Array.from({ length: maxJobs }, (_, i) => i + 1).map((n) => (
                    <option key={n} value={n}>{n}</option>
                  ))}
                </select>
              </div>
              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={buildIgnoreErrors}
                  onChange={(e) => setBuildIgnoreErrors(e.target.checked)}
                  className="w-4 h-4 rounded border-slate-600 bg-slate-800 text-emerald-500 focus:ring-emerald-500 focus:ring-offset-0"
                />
                <span className="text-sm text-slate-300">Ignore errors (continue on failure)</span>
              </label>
              <p className="text-xs text-slate-500">Like make <code className="px-1 py-0.5 rounded bg-slate-800">-i</code>: if a target fails, keep building the rest.</p>
            </div>
            <div className="px-6 py-4 flex justify-end gap-3 border-t border-slate-700/50 bg-slate-900/50">
              <button
                onClick={() => setShowBuildModal(false)}
                className="px-4 py-2 rounded-lg text-slate-300 hover:text-white hover:bg-slate-800 text-sm font-medium transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => handleBuild(buildModalClean, buildJobs, buildIgnoreErrors)}
                className="px-5 py-2 rounded-lg bg-gradient-to-r from-emerald-500 via-teal-500 to-cyan-600 hover:from-emerald-600 hover:via-teal-600 hover:to-cyan-700 text-white text-sm font-semibold transition-colors"
              >
                {buildModalClean ? "Clean & Build" : "Run Build"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Error Banner */}
      {parseError && (
        <div className="mx-8 mt-4 px-5 py-3.5 rounded-xl bg-red-950/40 border border-red-800/50 text-red-200 text-sm flex items-center gap-3 shadow-lg backdrop-blur-sm animate-slide-down">
          <svg
            className="w-5 h-5 flex-shrink-0 text-red-400"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
          <span className="flex-1">{parseError}</span>
          <button
            onClick={() => setParseError(null)}
            className="text-red-400 hover:text-red-300 transition-colors"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      )}

      {/* Modern Tab Navigation */}
      {configPath && (
        <nav className="flex gap-2 px-8 py-4 bg-slate-900/40 backdrop-blur-xl border-b border-slate-800/50">
          <TabButton
            active={activeTab === "graph"}
            onClick={() => setActiveTab("graph")}
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7" />
              </svg>
            }
          >
            Dependency Graph
          </TabButton>
          <TabButton
            active={activeTab === "files"}
            onClick={() => setActiveTab("files")}
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
              </svg>
            }
            badge={buildTomlFiles.length > 0 && (
              <span className="ml-2 px-2 py-0.5 rounded-full text-xs font-semibold bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
                {buildTomlFiles.length}
              </span>
            )}
          >
            Project Files
          </TabButton>
          <TabButton
            active={activeTab === "build"}
            onClick={() => setActiveTab("build")}
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
            }
            badge={buildLog.length > 0 && (
              <span className={`ml-2 px-2 py-0.5 rounded-full text-xs font-semibold border ${
                buildSuccess === true ? "bg-green-500/20 text-green-400 border-green-500/30" :
                buildSuccess === false ? "bg-red-500/20 text-red-400 border-red-500/30" :
                "bg-slate-700/50 text-slate-400 border-slate-600/30"
              }`}>
                {buildLog.length}
              </span>
            )}
          >
            Build Output
          </TabButton>
        </nav>
      )}

      {/* Main Content */}
      <main className="flex-1 min-h-0 overflow-hidden relative">
        {!configPath ? (
          <>
            <WelcomeScreen
              onCreateNew={handleCreateNew}
              onOpenFile={handleFilePicker}
              onConvertCmake={handleConvertCmake}
              buildRunning={buildRunning}
            />
            <NewProjectModal
              isOpen={showNewProjectModal}
              onClose={() => setShowNewProjectModal(false)}
              onCreate={handleCreateProject}
            />
          </>
        ) : activeTab === "graph" ? (
          <DependencyGraph projectInfo={projectInfo} />
        ) : activeTab === "files" ? (
          <div className="h-full flex min-h-0">
            <div className="w-72 flex-shrink-0 border-r border-slate-800 overflow-hidden flex flex-col">
              <FileTree
                projectRoot={projectRoot || (configPath ? configPath.split("/").slice(0, -1).join("/") : null)}
                buildTomlFiles={buildTomlFiles}
                projectInfo={projectInfo}
                configPath={configPath}
                onFileSelect={handleFileSelect}
              />
            </div>
            <div className="flex-1 min-w-0 flex flex-col">
              {configPath ? (
                <BuildTomlEditor
                  content={tomlContent}
                  onChange={setTomlContent}
                  onSave={handleSaveToml}
                />
              ) : (
                <div className="h-full flex items-center justify-center text-slate-500 text-sm">
                  Select a build.toml to edit
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="h-full min-h-0 flex flex-col overflow-hidden">
            <BuildLog
              lines={buildLog}
              success={buildSuccess}
              buildRunning={buildRunning}
              onCancelBuild={handleCancelBuild}
              className="flex-1 min-h-0"
            />
          </div>
        )}
      </main>
    </div>
  );
}

function WelcomeScreen({ onCreateNew, onOpenFile, onConvertCmake, buildRunning }) {
  return (
    <div className="h-full flex flex-col items-center justify-center p-6 relative overflow-y-auto">
      {/* Animated Background */}
      <div className="absolute inset-0 overflow-hidden">
        <div className="absolute top-0 left-1/4 w-96 h-96 bg-emerald-500/10 rounded-full blur-3xl animate-pulse-slow"></div>
        <div className="absolute bottom-0 right-1/4 w-96 h-96 bg-red-600/10 rounded-full blur-3xl animate-pulse-slow" style={{ animationDelay: '1s' }}></div>
        <div className="absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] bg-gradient-to-r from-emerald-500/5 via-teal-500/5 to-cyan-600/5 rounded-full blur-3xl"></div>
      </div>

      <div className="relative z-10 max-w-4xl w-full space-y-6 animate-fade-in py-6">
        {/* Hero Section */}
        <div className="text-center space-y-2">
          <div className="inline-flex items-center justify-center mb-2">
            <div className="relative w-12 h-12 rounded-xl bg-gradient-to-br from-emerald-500 via-teal-500 to-cyan-600 flex items-center justify-center shadow-lg shadow-emerald-500/20">
              <svg className="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
              </svg>
            </div>
          </div>
          <h1 className="text-2xl md:text-3xl font-bold bg-gradient-to-r from-white to-slate-300 bg-clip-text text-transparent">
            Welcome to ngmake
          </h1>
          <p className="text-sm text-slate-400 max-w-xl mx-auto">
            Modern C++ build tool with TOML configuration
          </p>
          <div className="flex items-center justify-center gap-2 pt-1">
            <div className="h-px w-8 bg-gradient-to-r from-transparent to-emerald-500/50"></div>
            <span className="text-sm font-semibold bg-gradient-to-r from-emerald-400 via-teal-400 to-cyan-500 bg-clip-text text-transparent">
              Make Builds Great Again
            </span>
            <div className="h-px w-8 bg-gradient-to-r from-cyan-500/50 to-transparent"></div>
          </div>
          <div className="inline-flex flex-wrap items-center justify-center gap-2 pt-2">
            <span className="px-2 py-0.5 rounded text-xs font-medium bg-slate-800/80 border border-slate-700 text-slate-400">No CMake</span>
            <span className="px-2 py-0.5 rounded text-xs font-medium bg-slate-800/80 border border-slate-700 text-slate-400">No Make</span>
            <span className="px-2 py-0.5 rounded text-xs font-medium bg-emerald-500/20 border border-emerald-500/30 text-emerald-400">Just ngmake</span>
          </div>
        </div>

        {/* Action Cards */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 px-2">
          {/* Create New */}
          <ActionCard
            onClick={onCreateNew}
            icon={
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
            }
            title="Create New Project"
            description="Fresh build.toml template"
            gradient="from-blue-500/20 to-cyan-500/20"
            iconBg="from-blue-500 to-cyan-500"
            delay="0s"
          />
          <ActionCard
            onClick={onOpenFile}
            icon={
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
              </svg>
            }
            title="Open Existing"
            description="Load a build.toml file"
            gradient="from-purple-500/20 to-pink-500/20"
            iconBg="from-purple-500 to-pink-500"
            delay="0.1s"
          />
          <ActionCard
            onClick={onConvertCmake}
            disabled={buildRunning}
            icon={buildRunning ? (
              <svg className="animate-spin w-5 h-5" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
              </svg>
            ) : (
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
            )}
            title={buildRunning ? "Converting..." : "Convert CMake"}
            description="CMakeLists.txt → build.toml"
            gradient="from-emerald-500/20 to-cyan-500/20"
            iconBg="from-emerald-500 to-cyan-500"
            badge={<span className="px-1.5 py-0.5 rounded text-[10px] font-semibold bg-teal-500/20 text-teal-400 border border-teal-500/30">BETA</span>}
            delay="0.2s"
          />
        </div>
      </div>
    </div>
  );
}

function ActionCard({ onClick, icon, title, description, gradient, iconBg, badge, disabled, delay = "0s" }) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className="group relative p-4 rounded-xl bg-slate-900/60 backdrop-blur-xl border border-slate-800/50 hover:border-slate-700 transition-all duration-200 hover:shadow-lg hover:shadow-emerald-500/5 disabled:opacity-50 disabled:cursor-not-allowed animate-slide-up cursor-pointer w-full z-10"
      style={{ animationDelay: delay }}
    >
      <div className={`absolute inset-0 rounded-xl bg-gradient-to-br ${gradient} opacity-0 group-hover:opacity-100 transition-opacity duration-200 pointer-events-none`}></div>
      <div className="relative z-10 flex flex-col items-center space-y-3">
        <div className={`w-10 h-10 rounded-lg bg-gradient-to-br ${iconBg} flex items-center justify-center text-white shadow group-hover:scale-105 transition-transform`}>
          {icon}
        </div>
        <div className="space-y-0.5 text-center">
          <div className="flex items-center justify-center gap-1.5 flex-wrap">
            <h3 className="text-sm font-semibold text-white">{title}</h3>
            {badge}
          </div>
          <p className="text-xs text-slate-400">{description}</p>
        </div>
      </div>
    </button>
  );
}

function TabButton({ active, onClick, children, icon, badge }) {
  return (
    <button
      onClick={onClick}
      className={`relative px-5 py-3 rounded-xl text-sm font-semibold transition-all duration-200 flex items-center gap-2 ${
        active
          ? "text-white bg-gradient-to-r from-emerald-500/20 to-cyan-600/20 border border-emerald-500/30 shadow-lg shadow-emerald-500/10"
          : "text-slate-400 hover:text-white hover:bg-slate-800/50 border border-transparent"
      }`}
    >
      {icon}
      <span>{children}</span>
      {badge}
      {active && (
        <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-gradient-to-r from-emerald-500 via-teal-500 to-cyan-600 rounded-full"></div>
      )}
    </button>
  );
}

export default App;
