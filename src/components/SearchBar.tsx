import { useLayoutEffect, useRef } from "react";
import { ChevronDown, ChevronUp, Search, X } from "lucide-react";
import "./search.css";

export interface SearchBarProps {
  query: string;
  onQueryChange(query: string): void;
  index: number;
  total: number;
  searching: boolean;
  error: string | null;
  hasText: boolean;
  onNext(): void;
  onPrevious(): void;
  onClose(): void;
}

export function SearchBar({ query, onQueryChange, index, total, searching, error, hasText, onNext, onPrevious, onClose }: SearchBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  useLayoutEffect(() => { inputRef.current?.focus(); inputRef.current?.select(); }, []);
  const status = !query.trim() ? "Find words or phrases" : searching ? "Searching…" : total > 0 ? `${Math.max(0, Math.min(index, total - 1)) + 1} of ${total}` : error ? "Search incomplete" : hasText ? "No matches" : "No searchable text";
  return <section className="search-bar" aria-label="Document search" onKeyDown={(event) => {
    event.stopPropagation();
    if (event.key === "Escape") { event.preventDefault(); onClose(); }
    else if (event.key === "Enter") { event.preventDefault(); if (total) { if (event.shiftKey) onPrevious(); else onNext(); } }
  }}>
    <div className="search-controls"><Search size={16} aria-hidden="true" />
      <input ref={inputRef} type="search" aria-label="Find in document" placeholder="Find in document" value={query} onChange={(event) => onQueryChange(event.target.value)} />
      <span className="search-count" role="status" aria-live="polite">{status}</span>
      <button className="icon-button" aria-label="Previous match" title="Previous match (Shift+Enter)" disabled={!total} onClick={onPrevious}><ChevronUp size={17} /></button>
      <button className="icon-button" aria-label="Next match" title="Next match (Enter)" disabled={!total} onClick={onNext}><ChevronDown size={17} /></button>
      <button className="icon-button" aria-label="Close search" title="Close search (Escape)" onClick={onClose}><X size={17} /></button>
    </div>
    {error && <p className="search-error" role="alert">{error}</p>}
  </section>;
}
