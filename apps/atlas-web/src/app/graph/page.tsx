import { KnowledgeGraphView } from "@/components/KnowledgeGraphView";

export default function GraphPage() {
  return (
    <div className="space-y-4">
      <section>
        <h1 className="text-2xl font-semibold tracking-tight mb-1">Knowledge Graph</h1>
        <p className="text-[var(--foreground-muted)]">
          Every node, edge, and annotation is a signed, hash-chained event.
          Click a node to see its provenance trail.
        </p>
      </section>

      <KnowledgeGraphView />
    </div>
  );
}
