import { useLayoutEffect, useRef } from "react";
import { ChevronDown, ChevronUp, Search, X } from "lucide-react";
import "./search.css";
import {DEFAULT_SEARCH_OPTIONS} from "../editor/search-matching";
import type {PageSearchOptions} from "../editor/types";

export interface SearchBarProps {
  query: string;
  onQueryChange(query: string): void;
  index: number;
  total: number;
  searching: boolean;
  error: string | null;
  hasText: boolean;
  options?:PageSearchOptions;
  onOptionsChange?(options:PageSearchOptions):void;
  focusRequest?:number;
  onNext(): void;
  onPrevious(): void;
  onClose(): void;
  onReplace?():void;
}

export function SearchBar({ query, onQueryChange, index, total, searching, error, hasText, options=DEFAULT_SEARCH_OPTIONS,onOptionsChange,focusRequest=0,onNext, onPrevious, onClose,onReplace }: SearchBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const navigationPending = !total || (searching && index >= total);
  useLayoutEffect(() => { inputRef.current?.focus(); inputRef.current?.select(); }, [focusRequest]);
  const status = !query.trim() ? "Find words or phrases" : searching ? "Searching…" : total > 0 ? `${Math.max(0, Math.min(index, total - 1)) + 1} of ${total}` : error ? "Search incomplete" : hasText ? "No matches" : "No searchable text";
  return <section className="search-bar" aria-label="Document search" onKeyDown={(event) => {
    if (event.nativeEvent.isComposing || event.keyCode === 229) {event.stopPropagation();return;}
    if(event.ctrlKey&&!event.metaKey&&!event.altKey&&event.key==="Tab")return;
    event.stopPropagation();
    if ((event.ctrlKey||event.metaKey)&&!event.altKey&&event.key.toLowerCase()==="f") {event.preventDefault();inputRef.current?.focus();inputRef.current?.select();}
    else if (onReplace&&(event.ctrlKey||event.metaKey)&&!event.altKey&&event.key.toLowerCase()==="h") {event.preventDefault();onReplace();}
    else if (event.key === "Escape") { event.preventDefault(); onClose(); }
    else if ((event.key === "Enter"||event.key==="F3")&&!event.ctrlKey&&!event.metaKey&&!event.altKey) { event.preventDefault(); if (!navigationPending) { if (event.shiftKey) onPrevious(); else onNext(); } }
  }}>
    <div className="search-controls"><Search size={16} aria-hidden="true" />
      <input ref={inputRef} type="search" aria-label="Find in document" placeholder="Find in document" value={query} onChange={(event) => onQueryChange(event.target.value)} />
      <span className="search-count" role="status" aria-live="polite">{status}</span>
      <button className="icon-button" aria-label="Previous match" title="Previous match (Shift+Enter or Shift+F3)" disabled={navigationPending} onClick={onPrevious}><ChevronUp size={17} /></button>
      <button className="icon-button" aria-label="Next match" title="Next match (Enter or F3)" disabled={navigationPending} onClick={onNext}><ChevronDown size={17} /></button>
      {onReplace&&<button type="button" aria-label="Replace text" title="Replace text (Ctrl+H)" onClick={onReplace}>Replace…</button>}
      <button className="icon-button" aria-label="Close search" title="Close search (Escape)" onClick={onClose}><X size={17} /></button>
    </div>
    {onOptionsChange&&<div className="search-options">
      <label><input type="checkbox" checked={options.matchCase} onChange={event=>onOptionsChange({...options,matchCase:event.target.checked})}/>Match case</label>
      <label><input type="checkbox" checked={options.wholeWords} onChange={event=>onOptionsChange({...options,wholeWords:event.target.checked})}/>Whole words</label>
    </div>}
    {error && <p className="search-error" role="alert">{error}</p>}
  </section>;
}
