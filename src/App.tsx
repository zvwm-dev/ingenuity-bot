function App() {
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
          tablets <span className="text-sub">/ runes of aldur</span>
        </span>
        <span className="ml-auto border border-a3 bg-adim px-2 py-[2px] font-mono text-[9px] tracking-[0.12em] text-a2 uppercase">
          scaffold
        </span>
      </header>

      {/* ── Body ── */}
      <main className="flex flex-1 flex-col items-center justify-center gap-3">
        <div className="font-mono text-[22px] text-dim">[ ]</div>
        <div className="text-center font-mono text-[11px] leading-[1.9] tracking-[0.14em] text-sub uppercase">
          ingenuity desktop shell is live
          <br />
          <span className="text-mid">tablet mod valuator — phase 2 scaffold</span>
        </div>
      </main>

      {/* ── Statusbar ── */}
      <footer className="flex h-[26px] flex-shrink-0 items-center gap-5 border-t border-border bg-bg4 px-[14px]">
        <span className="font-mono text-[9px] tracking-[0.1em] text-dim">
          trade data <span className="text-mid">not connected</span>
        </span>
        <span className="ml-auto font-mono text-[9px] tracking-[0.1em] text-dim">
          not affiliated with or endorsed by Grinding Gear Games
        </span>
      </footer>
    </div>
  );
}

export default App;
