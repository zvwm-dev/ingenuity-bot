import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Confidence, ModRow, Valuation } from "./types";

const LEAGUE = "Runes of Aldur";
type Unit = "exalted" | "divine";
type SortKey = "value" | "sample";

export default function App() {
  const [data, setData] = useState<Valuation | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [unit, setUnit] = useState<Unit>("exalted");
  const [search, setSearch] = useState("");
  const [typeFilter, setTypeFilter] = useState<string>("all");
  const [sortKey, setSortKey] = useState<SortKey>("value");
  const [selected, setSelected] = useState<ModRow | null>(null);

  async function load(refresh: boolean) {
    setLoading(true);
    setError(null);
    try {
      const v = await invoke<Valuation>("value_tablets", { league: LEAGUE, refresh });
      setData(v);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load(false);
  }, []);

  const divRate = data?.divine_to_exalted ?? null;
  const canDivine = divRate != null && divRate > 0;

  const rows = useMemo<ModRow[]>(() => {
    if (!data) return [];
    const flat: ModRow[] = data.types.flatMap((t) =>
      t.mods.map((m) => ({ ...m, tablet_type: t.tablet_type, type_r2: t.r2 })),
    );
    const q = search.trim().toLowerCase();
    return flat
      .filter((r) => (typeFilter === "all" ? true : r.tablet_type === typeFilter))
      .filter((r) => (q ? r.description.toLowerCase().includes(q) : true))
      .sort((a, b) =>
        sortKey === "value"
          ? b.value_exalted - a.value_exalted
          : b.sample_size - a.sample_size,
      );
  }, [data, search, typeFilter, sortKey]);

  const fmt = (ex: number) => {
    if (unit === "divine" && canDivine) {
      const d = ex / (divRate as number);
      return { num: d.toFixed(d >= 10 ? 0 : 2), unit: "div" };
    }
    return { num: ex >= 10 ? ex.toFixed(0) : ex.toFixed(1), unit: "ex" };
  };

  const types = data?.types.map((t) => t.tablet_type) ?? [];
  const updated = data ? new Date(data.updated_at).toLocaleString() : "—";

  return (
    <div className="flex h-screen flex-col">
      {/* ── Titlebar ── */}
      <header className="flex h-[38px] flex-shrink-0 items-center gap-[10px] border-b border-border bg-bg3 px-4 select-none">
        <span className="h-[6px] w-[6px] flex-shrink-0 rounded-full bg-accent" />
        <span className="font-mono text-[13px]">
          <span className="font-light text-accent">[</span>
          <span className="text-text">ingenuity</span>
          <span className="font-light text-accent">]</span>
        </span>
        <span className="mx-[6px] h-4 w-px bg-border2" />
        <span className="font-mono text-[10px] text-mid">
          tablet mods <span className="text-sub">/ {LEAGUE.toLowerCase()}</span>
        </span>
        <div className="ml-auto flex items-center gap-3">
          {/* currency toggle */}
          <div className="flex border border-border2 font-mono text-[9px] uppercase">
            {(["exalted", "divine"] as Unit[]).map((u) => (
              <button
                key={u}
                onClick={() => setUnit(u)}
                disabled={u === "divine" && !canDivine}
                className={`px-2 py-[3px] tracking-[0.1em] disabled:opacity-30 ${
                  unit === u ? "bg-adim text-accent" : "text-mid hover:text-sub"
                }`}
              >
                {u === "exalted" ? "ex" : "div"}
              </button>
            ))}
          </div>
          <button
            onClick={() => load(true)}
            disabled={loading}
            className="border border-a3 bg-adim px-2 py-[3px] font-mono text-[9px] tracking-[0.12em] text-a2 uppercase hover:text-accent disabled:opacity-40"
          >
            {loading ? "···" : "refresh"}
          </button>
        </div>
      </header>

      {/* ── Filterbar ── */}
      <div className="flex h-[40px] flex-shrink-0 items-center gap-2 overflow-x-auto border-b border-border bg-bg3 px-[14px]">
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="search mods…"
          className="w-[220px] flex-shrink-0 border border-border2 bg-bg px-[10px] py-[5px] font-mono text-[11px] text-text outline-none placeholder:text-mid focus:border-a3"
        />
        <Pill active={typeFilter === "all"} onClick={() => setTypeFilter("all")}>
          all
        </Pill>
        {types.map((t) => (
          <Pill key={t} active={typeFilter === t} onClick={() => setTypeFilter(t)}>
            {t.replace(" Tablet", "")}
          </Pill>
        ))}
        <span className="ml-auto flex-shrink-0 pl-3 font-mono text-[9px] tracking-[0.1em] text-dim">
          sort:{" "}
          <button
            className={sortKey === "value" ? "text-accent" : "text-mid"}
            onClick={() => setSortKey("value")}
          >
            value
          </button>{" "}
          ·{" "}
          <button
            className={sortKey === "sample" ? "text-accent" : "text-mid"}
            onClick={() => setSortKey("sample")}
          >
            sample
          </button>
        </span>
      </div>

      {/* ── Body ── */}
      <div className="flex flex-1 overflow-hidden">
        {/* list */}
        <div className="flex flex-1 flex-col overflow-hidden">
          <div className="grid flex-shrink-0 grid-cols-[1fr_92px_64px_48px_84px] border-b border-border bg-bg4 px-[14px] py-[7px] font-mono text-[9px] tracking-[0.14em] text-mid uppercase select-none">
            <span>Modifier</span>
            <span>Type</span>
            <span className="text-right">Value</span>
            <span className="text-right">n</span>
            <span className="text-right">Conf</span>
          </div>

          <div className="flex-1 overflow-y-auto">
            {loading && <Banner>computing valuations… first load pulls live data, ~1–2 min</Banner>}
            {error && !loading && <Banner tone="bad">error: {error}</Banner>}
            {!loading && !error && rows.length === 0 && <Banner>no mods match</Banner>}
            {!loading &&
              rows.map((r) => {
                const v = fmt(r.value_exalted);
                const active = selected?.stat_hash === r.stat_hash && selected?.tablet_type === r.tablet_type;
                return (
                  <div
                    key={`${r.tablet_type}:${r.stat_hash}`}
                    onClick={() => setSelected(r)}
                    className={`grid cursor-pointer grid-cols-[1fr_92px_64px_48px_84px] items-center gap-2 border-b border-border border-l-2 px-[14px] py-[9px] ${
                      active ? "border-l-accent bg-[#0c1812]" : "border-l-transparent hover:bg-bg3"
                    }`}
                  >
                    <span className="truncate font-sans text-[12px] text-text">
                      <span className={r.affix === "Prefix" ? "text-sub" : "text-mid"}>
                        {r.affix === "Prefix" ? "P " : r.affix === "Suffix" ? "S " : "  "}
                      </span>
                      {r.description}
                    </span>
                    <span className="truncate font-mono text-[10px] text-mid">
                      {r.tablet_type.replace(" Tablet", "")}
                    </span>
                    <span className="text-right font-mono text-[12px] font-medium text-bright">
                      {v.num}
                      <span className="ml-[2px] text-[9px] font-light text-sub">{v.unit}</span>
                    </span>
                    <span className="text-right font-mono text-[11px] text-mid">{r.sample_size}</span>
                    <span className="text-right">
                      <ConfBadge c={r.confidence} />
                    </span>
                  </div>
                );
              })}
          </div>
        </div>

        {/* detail */}
        <div className="w-[300px] flex-shrink-0 overflow-y-auto border-l border-border bg-bg3">
          {selected ? (
            <Detail row={selected} fmt={fmt} />
          ) : (
            <div className="flex h-full flex-col items-center justify-center gap-[10px] p-8 text-center">
              <div className="font-mono text-[22px] text-dim">[ ]</div>
              <div className="font-mono text-[10px] leading-[1.8] tracking-[0.14em] text-dim uppercase">
                select a mod
                <br />
                to see its value
              </div>
            </div>
          )}
        </div>
      </div>

      {/* ── Statusbar ── */}
      <footer className="flex h-[26px] flex-shrink-0 items-center gap-5 border-t border-border bg-bg4 px-[14px] font-mono text-[9px] tracking-[0.1em]">
        <span className="text-dim">
          trade data <span className={error ? "text-down" : "text-accent"}>{error ? "error" : loading ? "loading" : "live"}</span>
        </span>
        <span className="text-dim">
          mods <span className="text-mid">{rows.length}</span>
        </span>
        <span className="text-dim">
          updated <span className="text-mid">{updated}</span>
        </span>
        <span className="ml-auto text-dim">not affiliated with or endorsed by Grinding Gear Games</span>
      </footer>
    </div>
  );
}

function Pill({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className={`flex-shrink-0 border px-[10px] py-[4px] font-mono text-[9px] tracking-[0.1em] uppercase ${
        active ? "border-a3 bg-adim text-accent" : "border-border2 text-mid hover:text-sub"
      }`}
    >
      {children}
    </button>
  );
}

function ConfBadge({ c }: { c: Confidence }) {
  const cls =
    c === "High"
      ? "border-accent text-accent"
      : c === "Medium"
        ? "border-mid text-sub"
        : "border-dim text-dim";
  return (
    <span className={`border px-[5px] py-[1px] font-mono text-[9px] ${cls}`}>{c.toLowerCase()}</span>
  );
}

function Banner({ children, tone }: { children: React.ReactNode; tone?: "bad" }) {
  return (
    <div
      className={`px-[14px] py-6 text-center font-mono text-[10px] tracking-[0.12em] uppercase ${
        tone === "bad" ? "text-down" : "text-dim"
      }`}
    >
      {children}
    </div>
  );
}

function Detail({ row, fmt }: { row: ModRow; fmt: (ex: number) => { num: string; unit: string } }) {
  const val = fmt(row.value_exalted);
  const lo = fmt(row.ci_low);
  const hi = fmt(row.ci_high);
  return (
    <div className="flex flex-col">
      <div className="border-b border-border p-[18px]">
        <div className="mb-2 font-mono text-[9px] tracking-[0.18em] text-mid uppercase">
          {row.affix} · {row.tablet_type}
        </div>
        <div className="font-sans text-[14px] leading-[1.35] font-medium text-bright">{row.description}</div>
      </div>
      <div className="flex flex-col gap-5 p-[18px]">
        <Cell label="Estimated value">
          <span className="font-mono text-[22px] font-medium text-bright">{val.num}</span>
          <span className="ml-1 text-[11px] text-sub">{val.unit}</span>
          <span className="ml-2 align-middle">
            <ConfBadge c={row.confidence} />
          </span>
        </Cell>
        <div className="grid grid-cols-2 gap-px bg-border">
          <Stat label="95% range">
            {lo.num}–{hi.num} {hi.unit}
          </Stat>
          <Stat label="Sample size">{row.sample_size} listings</Stat>
          <Stat label="Per unit roll">{row.per_unit_exalted.toFixed(2)} ex</Stat>
          <Stat label="Typical roll">{row.typical_roll.toFixed(0)}</Stat>
        </div>
        <div className="border border-border bg-bg2 p-3 font-mono text-[9px] leading-[1.7] text-mid">
          additive model fit (this type): R² {row.type_r2.toFixed(2)}
          <br />
          <span className="text-dim">
            value = avg marginal price of this mod. combos can sell above the sum of parts; low
            R² / wide range = trust less.
          </span>
        </div>
      </div>
    </div>
  );
}

function Cell({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-[6px]">
      <div className="font-mono text-[8px] tracking-[0.18em] text-mid uppercase">{label}</div>
      <div>{children}</div>
    </div>
  );
}

function Stat({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-[5px] bg-bg2 p-3">
      <div className="font-mono text-[8px] tracking-[0.16em] text-mid uppercase">{label}</div>
      <div className="font-mono text-[13px] text-bright">{children}</div>
    </div>
  );
}
