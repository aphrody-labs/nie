// Builds replica/design/tokens.<theme>.json from packages/rg-ui/src/tokens.json (single source of colour).
const src = await Bun.file(new URL("../../packages/rg-ui/src/tokens.json", import.meta.url)).json();
const tpl = await Bun.file(process.argv[2]).json();
const out: string[] = [];
for (const [name, t] of Object.entries<any>(src.themes)) {
  const m = t.md3;
  const color = {
    bg: m.surface, surface: m.surfaceContainerLow, border: m.outlineVariant, "border-input": m.outline,
    text: m.onSurface, "text-muted": m.onSurfaceVariant, accent: m.primary, "on-accent": m.onPrimary,
    danger: m.error, "on-danger": m.onError,
  };
  const pairs = [["text","bg"],["text","surface"],["text-muted","bg"],["text-muted","surface"],["on-accent","accent"],["accent","bg"],["accent","surface"],["danger","bg"],["on-danger","danger"],["border-input","bg","ui"]];
  const file = new URL(`./tokens.${name}.json`, import.meta.url);
  await Bun.write(file, JSON.stringify({ ...tpl, _about: `Generated from packages/rg-ui/src/tokens.json, theme ${name}.`, color, pairs }, null, 2) + "\n");
  out.push(name);
}
console.log(out.join(" "));
