import { useEffect, useState } from "react";
import type { FolioAdapter } from "../editor/adapter";
import type { EditableTextRun, PagePlan, TextRuns, FormTextRuns } from "../editor/types";
import "./existing-text.css";

interface ExistingTextLayerProps {
  adapter: FolioAdapter;
  page: PagePlan;
  disabled?: boolean;
  onEditText?(run: EditableTextRun): void;
}

export function ExistingTextLayer({ adapter, page, disabled, onEditText }: ExistingTextLayerProps) {
  const key = `${page.sourceId}:${page.pageIndex}`;
  const [state, setState] = useState<{ key: string; adapter: FolioAdapter; result?: TextRuns }>({ key, adapter });
  useEffect(() => {
    let active = true;
    setState({ key, adapter });
    const load = async () => {
      try {
        const [top, forms] = await Promise.allSettled([
          adapter.listTextRuns?.(page.sourceId,page.pageIndex) ?? Promise.resolve({runs:[]}),
          adapter.listFormTextRuns?.(page.sourceId,page.pageIndex) ?? Promise.resolve({runs:[]}),
        ]);
        const topResult: TextRuns = top.status === "fulfilled" ? top.value : {runs:[],reason:String(top.reason instanceof Error ? top.reason.message : top.reason)};
        const formResult: FormTextRuns = forms.status === "fulfilled" ? forms.value : {runs:[],reason:String(forms.reason instanceof Error ? forms.reason.message : forms.reason)};
        const result: TextRuns = {
          runs:[...topResult.runs,...formResult.runs.map(run=>({...run,objectIndex:run.objectPath[0],canSubstitute:false}))],
          reason:formResult.reason ?? topResult.reason,
        };
        if (active) setState({ key, adapter, result });
      } catch (error) {
        if (active) setState({ key, adapter, result: { runs: [], reason: error instanceof Error ? error.message : String(error) } });
      }
    };
    void load();
    return () => { active = false; };
  }, [adapter, key, page.sourceId, page.pageIndex]);
  const result = state.key === key && state.adapter === adapter ? state.result : undefined;
  const runs = result?.runs.filter(run => run.bounds.width > 0 && run.bounds.height > 0) ?? [];
  const message = !result ? "Finding text…" : !runs.length
    ? result.reason ?? "No editable text on this page. Scanned pages need OCR, which is not available yet." : undefined;
  return <g className="existing-text-layer" onPointerDown={event => event.stopPropagation()}>
    {runs.map(run => <rect key={run.objectPath ? `form:${run.objectPath.join(".")}` : `text:${run.objectIndex}`} {...run.bounds} className={`existing-text-run${run.supported ? "" : " unsupported"}`}
      data-text-object={run.objectPath ? undefined : run.objectIndex} data-text-path={run.objectPath?.join(".")} role="button" tabIndex={disabled ? -1 : 0} aria-disabled={disabled || undefined}
      aria-label={`${run.supported ? "Edit text" : "Inspect unsupported text"}: ${run.text}`}
      onClick={event => { event.stopPropagation(); if (!disabled) onEditText?.(run); }}
      onKeyDown={event => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); event.stopPropagation(); if (!disabled) onEditText?.(run); } }}>
      <title>{run.supported ? "Edit this text" : run.reason ?? "This text cannot be edited safely."}</title>
    </rect>)}
    {message && <foreignObject x={12} y={12} width={Math.max(1, page.width - 24)} height={Math.min(120, page.height - 24)} pointerEvents="none">
      <div className="existing-text-cue" role="status">{message}</div>
    </foreignObject>}
  </g>;
}
