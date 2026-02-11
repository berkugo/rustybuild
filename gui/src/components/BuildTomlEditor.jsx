import { useState } from "react";

export default function BuildTomlEditor({ content, onChange, onSave }) {
  const [hasChanges, setHasChanges] = useState(false);
  const [saveStatus, setSaveStatus] = useState(null);

  const handleChange = (value) => {
    setHasChanges(value !== content);
    onChange(value);
  };

  const handleSave = async () => {
    try {
      await onSave();
      setHasChanges(false);
      setSaveStatus("saved");
      setTimeout(() => setSaveStatus(null), 2000);
    } catch (e) {
      setSaveStatus("error");
      setTimeout(() => setSaveStatus(null), 2000);
    }
  };

  return (
    <div className="h-full flex flex-col bg-black">
      <div className="flex justify-between items-center px-4 py-3 border-b bg-surface-800 backdrop-blur-sm" style={{ borderBottomColor: 'rgba(154, 46, 21, 0.2)' }}>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2">
            <svg
              className="w-5 h-5 text-rust-400"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"
              />
            </svg>
            <span className="text-white font-medium">build.toml Editor</span>
          </div>
          {hasChanges && (
            <span className="px-2 py-1 rounded text-xs bg-emerald-900/30 text-emerald-300 border border-emerald-500/50">
              Unsaved changes
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {saveStatus === "saved" && (
            <span className="px-3 py-1.5 rounded-lg text-sm text-green-400 bg-green-900/30 border border-green-500/50 flex items-center gap-2">
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
              Saved
            </span>
          )}
          {saveStatus === "error" && (
            <span className="px-3 py-1.5 rounded-lg text-sm text-red-400 bg-red-900/30 border border-red-500/50 flex items-center gap-2">
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
              Error
            </span>
          )}
          <button
            onClick={handleSave}
            disabled={!hasChanges}
            className="px-5 py-2 rounded-lg bg-rust-500 hover:bg-rust-600 text-white font-medium text-sm shadow-md hover:shadow-lg transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
          >
            <svg
              className="w-4 h-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M5 13l4 4L19 7"
              />
            </svg>
            Save
          </button>
        </div>
      </div>
      <textarea
        value={content}
        onChange={(e) => handleChange(e.target.value)}
        className="flex-1 w-full resize-none p-6 bg-black text-white font-mono text-sm leading-relaxed focus:outline-none focus:ring-0 border-0"
        spellCheck={false}
        placeholder="# build.toml content..."
        style={{
          tabSize: 2,
        }}
      />
    </div>
  );
}

