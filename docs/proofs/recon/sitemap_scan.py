# /// script
# dependencies = ["httpx[http2]", "selectolax"]
# ///
"""Crawl every URL listed in a sitemap (and nested sitemap indexes), check each page by content."""
import asyncio, json, re, sys, time
import httpx
from selectolax.lexbor import LexborHTMLParser as HTMLParser

ROOT = sys.argv[1].rstrip("/")
OUT = sys.argv[2]
UA = "rosegriffon-sitemap-scan"


async def sitemap_urls(c, url, seen):
    if url in seen:
        return []
    seen.add(url)
    r = await c.get(url)
    locs = re.findall(r"<loc>\s*([^<\s]+)\s*</loc>", r.text)
    if "<sitemapindex" in r.text:
        out = []
        for l in locs:
            out += await sitemap_urls(c, l, seen)
        return out
    return locs


async def check(c, sem, url):
    async with sem:
        t = time.perf_counter()
        try:
            r = await c.get(url)
        except Exception as e:
            return {"url": url, "error": repr(e)}
        ms = round((time.perf_counter() - t) * 1000)
        row = {"url": url, "status": r.status_code, "final": str(r.url), "ms": ms, "bytes": len(r.content),
               "type": r.headers.get("content-type", "")}
        if "html" in row["type"]:
            h = HTMLParser(r.text)
            title = h.css_first("title")
            h1 = h.css_first("h1")
            canon = h.css_first('link[rel="canonical"]')
            desc = h.css_first('meta[name="description"]')
            robots = h.css_first('meta[name="robots"]')
            row |= {
                "title": title.text(strip=True) if title else None,
                "h1": h1.text(strip=True)[:120] if h1 else None,
                "canonical": canon.attributes.get("href") if canon else None,
                "description": bool(desc),
                "noindex": bool(robots and "noindex" in (robots.attributes.get("content") or "")),
                "text_chars": len((h.body.text(strip=True) if h.body else "")),
                "error_page": bool(re.search(r"404|introuvable|not found|Application error", (title.text() if title else "") + (h1.text() if h1 else ""), re.I)),
            }
        return row


async def main():
    async with httpx.AsyncClient(http2=True, follow_redirects=True, timeout=30, headers={"user-agent": UA}) as c:
        urls = sorted(set(await sitemap_urls(c, ROOT + "/sitemap.xml", set())))
        sem = asyncio.Semaphore(8)
        rows = await asyncio.gather(*(check(c, sem, u) for u in urls))
    json.dump(rows, open(OUT, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    bad = [r for r in rows if r.get("error") or r.get("status") != 200 or r.get("error_page") or not r.get("title")
           or (r.get("canonical") and r["canonical"].rstrip("/") != r["url"].rstrip("/")) or r.get("noindex")]
    print(f"urls={len(rows)} ok={len(rows)-len(bad)} issues={len(bad)}")
    print(f"slowest: {sorted(rows, key=lambda r: -r.get('ms', 0))[:5] and [(r['url'], r.get('ms')) for r in sorted(rows, key=lambda r: -r.get('ms', 0))[:5]]}")
    for r in bad:
        print(json.dumps({k: r.get(k) for k in ("url", "status", "final", "title", "h1", "canonical", "noindex", "error")}, ensure_ascii=False))


asyncio.run(main())
