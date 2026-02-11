import { useState } from "react";

export default function NewProjectModal({ isOpen, onClose, onCreate }) {
  const [projectName, setProjectName] = useState("");
  const [cppVersion, setCppVersion] = useState("17");
  const [projectType, setProjectType] = useState("executable");

  if (!isOpen) return null;

  const handleCreate = () => {
    if (!projectName.trim()) return;
    onCreate({
      name: projectName.trim(),
      cppVersion,
      projectType,
    });
    setProjectName("");
    setCppVersion("17");
    setProjectType("executable");
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-fade-in">
      <div className="relative w-full max-w-2xl bg-slate-900 rounded-2xl border border-slate-800 shadow-2xl animate-scale-in">
        {/* Header */}
        <div className="px-8 py-6 border-b border-slate-800">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-2xl font-bold text-white mb-1">Create New Project</h2>
              <p className="text-sm text-slate-400">Set up your C++ project structure</p>
            </div>
            <button
              onClick={onClose}
              className="p-2 rounded-lg hover:bg-slate-800 text-slate-400 hover:text-white transition-colors"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="px-8 py-6 space-y-6">
          {/* Project Name */}
          <div>
            <label className="block text-sm font-semibold text-slate-200 mb-2">
              Project Name
            </label>
            <input
              type="text"
              value={projectName}
              onChange={(e) => setProjectName(e.target.value)}
              placeholder="my_awesome_project"
              className="w-full px-4 py-3 rounded-xl bg-slate-800/50 border border-slate-700 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/50 focus:border-emerald-500/50 transition-all"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === "Enter" && projectName.trim()) {
                  handleCreate();
                }
              }}
            />
            <p className="mt-1.5 text-xs text-slate-500">This will be used as the project directory name</p>
          </div>

          {/* C++ Version */}
          <div>
            <label className="block text-sm font-semibold text-slate-200 mb-2">
              C++ Standard
            </label>
            <div className="relative">
              <select
                value={cppVersion}
                onChange={(e) => setCppVersion(e.target.value)}
                className="w-full px-4 py-3 rounded-xl bg-slate-800/50 border border-slate-700 text-white focus:outline-none focus:ring-2 focus:ring-emerald-500/50 focus:border-emerald-500/50 transition-all appearance-none cursor-pointer hover:border-slate-600"
              >
                <option value="11" className="bg-slate-800 text-white">C++11</option>
                <option value="14" className="bg-slate-800 text-white">C++14</option>
                <option value="17" className="bg-slate-800 text-white">C++17</option>
                <option value="20" className="bg-slate-800 text-white">C++20</option>
                <option value="23" className="bg-slate-800 text-white">C++23</option>
              </select>
              <div className="absolute inset-y-0 right-0 flex items-center pr-4 pointer-events-none">
                <svg
                  className="w-5 h-5 text-slate-400"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M19 9l-7 7-7-7"
                  />
                </svg>
              </div>
            </div>
          </div>

          {/* Project Type */}
          <div>
            <label className="block text-sm font-semibold text-slate-200 mb-2">
              Project Type
            </label>
            <div className="grid grid-cols-3 gap-3">
              {[
                { value: "executable", label: "Executable", icon: "▶", desc: "Application" },
                { value: "library", label: "Library", icon: "📚", desc: "Static/Shared" },
                { value: "mixed", label: "Mixed", icon: "🔀", desc: "Both" },
              ].map((type) => (
                <button
                  key={type.value}
                  onClick={() => setProjectType(type.value)}
                  className={`p-4 rounded-xl border transition-all ${
                    projectType === type.value
                      ? "bg-gradient-to-br from-emerald-500/20 to-cyan-600/20 border-emerald-500/50 shadow-lg shadow-emerald-500/10"
                      : "bg-slate-800/50 border-slate-700 hover:border-slate-600"
                  }`}
                >
                  <div className="text-2xl mb-2">{type.icon}</div>
                  <div className="font-semibold text-white text-sm">{type.label}</div>
                  <div className="text-xs text-slate-400 mt-1">{type.desc}</div>
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="px-8 py-6 border-t border-slate-800 flex items-center justify-end gap-3">
          <button
            onClick={onClose}
            className="px-5 py-2.5 rounded-xl bg-slate-800/50 hover:bg-slate-800 border border-slate-700 text-slate-300 hover:text-white font-medium text-sm transition-all"
          >
            Cancel
          </button>
          <button
            onClick={handleCreate}
            disabled={!projectName.trim()}
            className="px-6 py-2.5 rounded-xl bg-gradient-to-r from-emerald-500 via-teal-500 to-cyan-600 hover:from-emerald-600 hover:via-teal-600 hover:to-cyan-700 text-white font-semibold text-sm transition-all disabled:opacity-50 disabled:cursor-not-allowed shadow-lg shadow-emerald-500/20 hover:shadow-xl hover:shadow-teal-500/30"
          >
            Create Project
          </button>
        </div>
      </div>
    </div>
  );
}

