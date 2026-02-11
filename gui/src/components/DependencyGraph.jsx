import { useMemo } from "react";
import ReactFlow, {
  Controls,
  MiniMap,
  Background,
  BackgroundVariant,
  Position,
  MarkerType,
} from "reactflow";
import "reactflow/dist/style.css";

const nodeTypes = ["executable", "static_lib", "shared_lib"];
// Palette: emerald / teal / cyan (matches app logo)
const colors = {
  executable: "#06b6d4", // cyan-500
  static_lib: "#14b8a6", // teal-500
  shared_lib: "#10b981", // emerald-500
};

const typeLabels = {
  executable: "Executable",
  static_lib: "Static Library",
  shared_lib: "Shared Library",
};

function buildLayout(nodes, edges) {
  const levels = new Map();
  for (const n of nodes) {
    const list = levels.get(n.level) ?? [];
    list.push(n.id);
    levels.set(n.level, list);
  }
  const sortedLevels = Array.from(levels.entries()).sort((a, b) => a[0] - b[0]);
  const gap = 320;
  const nodeWidth = 240;
  const nodeHeight = 100;

  const nodeMap = new Map();
  sortedLevels.forEach(([level, ids], li) => {
    const totalHeight = ids.length * (nodeHeight + 50);
    const startY = -totalHeight / 2;
    ids.forEach((id, i) => {
      nodeMap.set(id, {
        x: li * gap,
        y: startY + i * (nodeHeight + 50),
      });
    });
  });

  const flowNodes = nodes.map((n) => {
    const nodeColor = colors[n.target_type] ?? "#14b8a6";
    return {
      id: n.id,
      type: "default",
      position: nodeMap.get(n.id) ?? { x: 0, y: 0 },
      data: {
        label: (
          <div className="flex flex-col items-center justify-center h-full w-full py-4 px-5">
            <div className="flex items-center gap-3 mb-2">
              <div
                className="w-4 h-4 rounded-full shadow-lg"
                style={{ 
                  backgroundColor: nodeColor,
                  boxShadow: `0 0 12px ${nodeColor}80`
                }}
              />
              <span className="font-bold text-white text-lg tracking-tight">{n.label}</span>
            </div>
            <span
              className="text-xs px-3 py-1.5 rounded-lg font-semibold uppercase tracking-wide"
              style={{
                color: nodeColor,
                backgroundColor: `${nodeColor}15`,
                border: `1.5px solid ${nodeColor}40`,
              }}
            >
              {typeLabels[n.target_type] || n.target_type}
            </span>
          </div>
        ),
      },
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      style: {
        borderColor: nodeColor,
        borderWidth: 2,
        width: nodeWidth,
        height: nodeHeight,
        background: "linear-gradient(145deg, rgb(30 41 59 / 0.95) 0%, rgb(15 23 42 / 0.98) 100%)",
        borderRadius: "16px",
        boxShadow: `
          0 8px 24px -4px ${nodeColor}30,
          0 0 0 1px rgb(51 65 85 / 0.6),
          inset 0 1px 0 rgba(255, 255, 255, 0.04)
        `,
      },
    };
  });

  const flowEdges = edges.map((e, i) => {
    const targetNode = nodes.find((n) => n.id === e.to);
    const edgeColor = colors[targetNode?.target_type] ?? "#14b8a6";
    return {
      id: `e-${e.from}-${e.to}-${i}`,
      source: e.from,
      target: e.to,
      type: "smoothstep",
      animated: true,
      style: {
        strokeWidth: 2.5,
        stroke: edgeColor,
        filter: `drop-shadow(0 0 4px ${edgeColor}60)`,
      },
      markerEnd: {
        type: MarkerType.ArrowClosed,
        color: edgeColor,
        width: 24,
        height: 24,
      },
    };
  });

  return { flowNodes, flowEdges };
}

export default function DependencyGraph({ projectInfo }) {
  const { flowNodes, flowEdges } = useMemo(() => {
    if (!projectInfo) return { flowNodes: [], flowEdges: [] };
    
    if (!projectInfo.graph_nodes || !projectInfo.graph_edges) {
      console.warn("Missing graph_nodes or graph_edges in projectInfo:", projectInfo);
      return { flowNodes: [], flowEdges: [] };
    }
    
    return buildLayout(projectInfo.graph_nodes, projectInfo.graph_edges);
  }, [projectInfo]);

  if (!projectInfo) {
    return (
      <div className="h-full flex items-center justify-center bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950">
        <div className="text-center space-y-6">
          <div className="w-24 h-24 mx-auto rounded-2xl bg-gradient-to-br from-emerald-500/20 via-teal-500/20 to-cyan-500/20 flex items-center justify-center backdrop-blur-sm border border-teal-500/30">
            <svg
              className="w-12 h-12 text-teal-400"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7"
              />
            </svg>
          </div>
          <div>
            <p className="text-slate-200 text-xl font-semibold mb-2">No Project Loaded</p>
            <p className="text-slate-400 text-sm">Load a build.toml file to visualize the dependency graph</p>
          </div>
        </div>
      </div>
    );
  }

  if (!projectInfo.graph_nodes || !projectInfo.graph_edges || 
      projectInfo.graph_nodes.length === 0) {
    return (
      <div className="h-full flex items-center justify-center bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950">
        <div className="text-center space-y-6">
          <div className="w-24 h-24 mx-auto rounded-2xl bg-gradient-to-br from-emerald-500/20 via-teal-500/20 to-cyan-500/20 flex items-center justify-center backdrop-blur-sm border border-teal-500/30">
            <svg
              className="w-12 h-12 text-teal-400"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7"
              />
            </svg>
          </div>
          <div>
            <p className="text-slate-200 text-xl font-semibold mb-2">No Targets Found</p>
            <p className="text-slate-400 text-sm">
              {projectInfo.project?.targets ? 
                `Found ${Object.keys(projectInfo.project.targets).length} targets but no graph data.` :
                "The project may need to be saved and reloaded."}
            </p>
          </div>
        </div>
      </div>
    );
  }

  const targetCount = Object.keys(projectInfo.project.targets).length;
  const executableCount = Object.values(projectInfo.project.targets).filter(
    (t) => t.target_type === "executable"
  ).length;
  const libCount = targetCount - executableCount;
  const depCount = flowEdges.length;

  return (
    <div className="h-full w-full relative bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950">
      {/* Project Info Card */}
      <div className="absolute top-6 left-6 z-10">
        <div className="rounded-2xl bg-slate-900/90 backdrop-blur-xl px-6 py-5 border border-slate-700/50 shadow-2xl">
          <div className="flex items-center gap-4 mb-5">
            <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-emerald-500/20 via-teal-500/20 to-cyan-500/20 flex items-center justify-center border border-teal-500/30">
              <svg
                className="w-6 h-6 text-teal-400"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
                />
              </svg>
            </div>
            <div>
              <h3 className="text-xl font-bold text-white mb-0.5">{projectInfo.project.name}</h3>
              <p className="text-xs text-slate-400 font-medium">v{projectInfo.project.version}</p>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3 pt-4 border-t border-slate-700/50">
            <div className="bg-slate-800/60 rounded-xl px-3 py-2.5 border border-slate-700/40">
              <div className="text-xs text-slate-400 mb-1 font-medium">Total Targets</div>
              <div className="text-2xl font-bold text-white tabular-nums">{targetCount}</div>
            </div>
            <div className="bg-slate-800/60 rounded-xl px-3 py-2.5 border border-slate-700/40">
              <div className="text-xs text-slate-400 mb-1 font-medium">Executables</div>
              <div className="text-2xl font-bold text-cyan-400 tabular-nums">{executableCount}</div>
            </div>
            <div className="bg-slate-800/60 rounded-xl px-3 py-2.5 border border-slate-700/40">
              <div className="text-xs text-slate-400 mb-1 font-medium">Libraries</div>
              <div className="text-2xl font-bold text-emerald-400 tabular-nums">{libCount}</div>
            </div>
            <div className="bg-slate-800/60 rounded-xl px-3 py-2.5 border border-slate-700/40">
              <div className="text-xs text-slate-400 mb-1 font-medium">Dependencies</div>
              <div className="text-2xl font-bold text-teal-400 tabular-nums">{depCount}</div>
            </div>
          </div>
        </div>
      </div>

      {/* Legend */}
      <div className="absolute top-6 right-6 z-10">
        <div className="rounded-2xl bg-slate-900/90 backdrop-blur-xl px-5 py-4 border border-slate-700/50 shadow-2xl">
          <h4 className="text-xs font-bold text-slate-400 mb-3 uppercase tracking-wider">Node Types</h4>
          <div className="space-y-2.5">
            {Object.entries(colors).map(([type, color]) => (
              <div key={type} className="flex items-center gap-3">
                <div
                  className="w-4 h-4 rounded-full border-2 flex-shrink-0"
                  style={{
                    backgroundColor: `${color}25`,
                    borderColor: color,
                    boxShadow: `0 0 10px ${color}40`
                  }}
                />
                <span className="text-sm text-slate-300 font-medium">{typeLabels[type] || type}</span>
              </div>
            ))}
          </div>
        </div>
      </div>

      <ReactFlow
        nodes={flowNodes}
        edges={flowEdges}
        fitView
        className="bg-transparent"
        defaultViewport={{ x: 0, y: 0, zoom: 0.8 }}
        minZoom={0.2}
        maxZoom={2}
        nodeTypes={{}}
      >
        <Background
          variant={BackgroundVariant.Dots}
          gap={28}
          size={1.5}
          color="rgb(71 85 105)"
          className="opacity-25"
        />
        <Controls
          className="!bg-slate-800/95 !border !border-slate-700/50 !rounded-xl !shadow-xl [&>button]:!bg-slate-800 [&>button]:!border-slate-600 [&>button]:!text-slate-300 [&>button:hover]:!bg-slate-700 [&>button:hover]:!text-teal-400"
          showInteractive={false}
          position="bottom-left"
        />
        <MiniMap
          className="!bg-slate-800/95 !border !border-slate-700/50 !rounded-xl !shadow-xl"
          nodeColor={(node) => {
            const targetType = projectInfo.graph_nodes?.find((n) => n.id === node.id)?.target_type;
            return colors[targetType] ?? "#14b8a6";
          }}
          maskColor="rgba(15, 23, 42, 0.85)"
          pannable
          zoomable
          position="bottom-right"
        />
      </ReactFlow>
    </div>
  );
}
