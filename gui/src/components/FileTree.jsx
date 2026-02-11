import { useState, useEffect, useMemo } from "react";

export default function FileTree({ projectRoot, buildTomlFiles = [], projectInfo, configPath, onFileSelect }) {
  const [expandedDirs, setExpandedDirs] = useState(new Set());

  const root = projectRoot ? projectRoot.replace(/\/$/, "") : "";

  const fileTree = useMemo(() => {
    if (!root) return null;

    const tree = { dirs: {}, files: [] };

    const addPath = (fullPath, type) => {
      let relativePath = fullPath.startsWith(root + "/") ? fullPath.slice(root.length + 1) : fullPath.replace(root, "").replace(/^\//, "");
      const parts = relativePath.split("/").filter(Boolean);
      if (parts.length === 0) return;
      let current = tree;
      for (let i = 0; i < parts.length - 1; i++) {
        const part = parts[i];
        if (!current.dirs[part]) current.dirs[part] = { dirs: {}, files: [] };
        current = current.dirs[part];
      }
      const name = parts[parts.length - 1];
      const existing = (current.files || []).find((f) => f.name === name && f.fullPath === fullPath);
      if (!existing) {
        if (!current.files) current.files = [];
        current.files.push({ name, fullPath, type });
      }
    };

    const tomlList = (buildTomlFiles && buildTomlFiles.length > 0) ? buildTomlFiles : (configPath ? [configPath] : []);
    tomlList.forEach((filePath) => addPath(filePath, "build_toml"));

    if (projectInfo?.project?.targets) {
      const targets = projectInfo.project.targets;
      const sourcePaths = new Set();
      Object.values(targets).forEach((t) => {
        (t.sources || []).forEach((s) => sourcePaths.add(s));
      });
      sourcePaths.forEach((fullPath) => addPath(fullPath, "source"));
    }

    return tree;
  }, [root, buildTomlFiles, configPath, projectInfo]);

  useEffect(() => {
    if (!fileTree) return;
    const allDirs = new Set();
    const collectDirs = (node, path) => {
      if (node.dirs) {
        Object.entries(node.dirs).forEach(([dirName, dirNode]) => {
          const dirPath = path ? `${path}/${dirName}` : dirName;
          allDirs.add(dirPath);
          collectDirs(dirNode, dirPath);
        });
      }
    };
    collectDirs(fileTree, "");
    setExpandedDirs(allDirs);
  }, [fileTree]);

  const toggleDir = (dirPath) => {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(dirPath)) next.delete(dirPath);
      else next.add(dirPath);
      return next;
    });
  };

  const renderTree = (node, path = "", level = 0) => {
    const items = [];

    if (node.dirs && Object.keys(node.dirs).length > 0) {
      Object.entries(node.dirs).forEach(([dirName, dirNode]) => {
        const dirPath = path ? `${path}/${dirName}` : dirName;
        const isExpanded = expandedDirs.has(dirPath);
        items.push(
          <div key={dirPath} style={{ paddingLeft: `${level * 16}px` }}>
            <div
              className="flex items-center gap-2 py-1 px-2 hover:bg-slate-700/50 rounded cursor-pointer text-slate-300 hover:text-white transition-colors"
              onClick={() => toggleDir(dirPath)}
            >
              <svg
                className={`w-4 h-4 transition-transform flex-shrink-0 ${isExpanded ? "rotate-90" : ""}`}
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
              </svg>
              <svg className="w-4 h-4 text-slate-500 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
              </svg>
              <span className="text-sm truncate">{dirName}</span>
            </div>
            {isExpanded && renderTree(dirNode, dirPath, level + 1)}
          </div>
        );
      });
    }

    if (node.files && node.files.length > 0) {
      node.files.forEach((file) => {
        const isBuildToml = file.type === "build_toml";
        const isSelected = configPath === file.fullPath;
        items.push(
          <div
            key={file.fullPath}
            style={{ paddingLeft: `${(level + 1) * 16}px` }}
            className={`flex items-center gap-2 py-1 px-2 rounded cursor-pointer group ${
              isBuildToml ? "text-emerald-400/90 hover:bg-slate-700/50 hover:text-emerald-300" : "text-slate-400 hover:bg-slate-700/30"
            } ${isSelected ? "bg-slate-700/50 text-emerald-300" : ""}`}
            onClick={() => onFileSelect && onFileSelect(file.fullPath, isBuildToml)}
          >
            {isBuildToml ? (
              <svg className="w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
              </svg>
            ) : (
              <svg className="w-4 h-4 flex-shrink-0 text-slate-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
            )}
            <span className="text-sm truncate">{file.name}</span>
          </div>
        );
      });
    }

    return items;
  };

  if (!fileTree || !root) {
    return (
      <div className="h-full flex items-center justify-center bg-slate-950 p-4">
        <p className="text-slate-500 text-sm">Import a build.toml or open a project to see structure.</p>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col bg-slate-950 overflow-hidden">
      <div className="p-3 border-b border-slate-800 flex-shrink-0">
        <h3 className="text-sm font-semibold text-white">Project structure</h3>
        <p className="text-xs text-slate-500 truncate mt-0.5" title={root}>
          {root}
        </p>
      </div>
      <div className="flex-1 overflow-y-auto p-2 space-y-0.5">
        {renderTree(fileTree)}
      </div>
    </div>
  );
}
