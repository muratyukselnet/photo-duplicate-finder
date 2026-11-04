import { withConfirmation } from "../lib/confirmation";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "../components/ui/button";
import { clearAiCache, getAiCapabilities, getAiSettings, setAiSettings } from "../lib/api";
import type { AiCapabilities, AiSettings } from "../lib/types";

export function SettingsPage() {
  const navigate = useNavigate();
  const [settings, setSettings] = useState<AiSettings | null>(null);
  const [capabilities, setCapabilities] = useState<AiCapabilities | null>(null);
  const [folders, setFolders] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    Promise.all([getAiSettings(), getAiCapabilities()]).then(([value, available]) => {
      setSettings(value); setFolders(value.protectedFolders.join("\n")); setCapabilities(available);
    }).catch((cause) => setError(String(cause)));
  }, []);
  const save = async () => {
    if (!settings) return;
    setBusy(true); setMessage(""); setError("");
    try { await setAiSettings({ ...settings, protectedFolders: folders.split("\n").map((item) => item.trim()).filter(Boolean) }); setMessage("Settings saved. Run AI review again to apply them."); }
    catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };
  const clear = async () => {
    setBusy(true); setMessage(""); setError("");
    try {
      await withConfirmation("Clear cached AI analysis and suggestions? Your photos and manual review decisions will be kept.", { title: "Clear AI cache", kind: "warning", okLabel: "Clear cache", cancelLabel: "Cancel" }, async () => {
      await clearAiCache(); setMessage("AI cache cleared. The next review will recompute analysis.");
      });
    }
    catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };
  return <main className="mx-auto max-w-3xl space-y-6 p-8">
    <header className="flex items-center justify-between"><div><p className="text-sm text-primary">Local only · $0</p><h1 className="mt-1 text-3xl font-semibold">AI review settings</h1></div><Button variant="ghost" onClick={() => navigate(-1)}>Back</Button></header>
    <p className="text-muted-foreground">Photos stay on your Mac. AI suggests keepers; you decide what moves to Trash.</p>
    {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
    {settings ? <>
      <section className="space-y-4 rounded-xl border border-border bg-card p-6">
        <label className="block space-y-2"><span className="font-medium">Selection preset</span><select className="block w-full rounded-md border border-border bg-background p-2" value={settings.preset} onChange={(event) => setSettings({ ...settings, preset: event.target.value as AiSettings["preset"] })}>
          <option value="conservative">Conservative</option><option value="balanced">Balanced</option><option value="aggressive">Aggressive</option>
        </select></label>
        <p className="text-sm text-muted-foreground">Conservative requires the strongest evidence. Balanced is the default. Aggressive accepts lower confidence. Protected photos always stay keepers.</p>
        <div className="border-t border-border pt-4"><p className="font-medium">Rules and protection · Always on</p><p className="mt-1 text-sm text-muted-foreground">Exact copies, edited photos, XMP ratings and RAW companions are checked before suggestions.</p></div>
        <label className="flex items-center gap-3"><input type="checkbox" className="h-4 w-4 accent-primary" disabled={!capabilities?.tier1Available || busy} checked={settings.tier1Enabled} onChange={(event) => setSettings({ ...settings, tier1Enabled: event.target.checked })} /><span>Tier 1 · Apple on-device analysis</span></label>
        <p className="text-sm text-muted-foreground">{capabilities?.tier1Available ? "Available on this Mac. No downloads, uploads or API keys." : `Unavailable: ${capabilities?.reason || "Apple analysis could not be detected."} Rules-only review is still available; uncertain groups need your review.`}</p>
        <label className="block space-y-2"><span className="font-medium">Protected folder names</span><textarea className="block min-h-24 w-full rounded-md border border-border bg-background p-3 text-sm" value={folders} onChange={(event) => setFolders(event.target.value)} placeholder={"Selects\nPicks"} /><span className="text-xs text-muted-foreground">One folder name per line. Photos in these folders are never suggested for removal.</span></label>
        <Button disabled={busy} onClick={save}>Save settings</Button>
      </section>
      <section className="flex items-center justify-between gap-6 rounded-xl border border-border p-6"><div><p className="font-medium">Analysis cache</p><p className="mt-1 text-sm text-muted-foreground">Unchanged photo content reuses local analysis across scans.</p></div><Button variant="secondary" disabled={busy} onClick={clear}>Clear AI cache</Button></section>
    </> : !error && <p role="status">Loading settings…</p>}
    {message && <p role="status" className="text-sm text-primary">{message}</p>}
  </main>;
}
