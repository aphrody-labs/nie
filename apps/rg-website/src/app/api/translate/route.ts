import { NextResponse } from "next/server";
import { translateIevrQuery } from "@rosegriffon/azalee/server/ecosystem";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const query = searchParams.get("q") ?? searchParams.get("text") ?? "";
  if (!query.trim()) {
    return NextResponse.json(
      { error: "Missing required query parameter `q`" },
      { status: 400 },
    );
  }

  const targetLang = searchParams.get("to") ?? searchParams.get("lang") ?? "fr";
  const skipRemote = searchParams.get("skipRemote") === "true";

  const result = await translateIevrQuery({
    text: query,
    targetLang,
    skipRemote,
  });

  return NextResponse.json(result, {
    headers: {
      "Cache-Control": "public, max-age=300, stale-while-revalidate=3600",
    },
  });
}

export async function POST(request: Request) {
  let body: {
    text?: unknown;
    q?: unknown;
    texts?: unknown;
    to?: unknown;
    lang?: unknown;
    skipRemote?: unknown;
  };
  try {
    body = (await request.json()) as typeof body;
  } catch {
    return NextResponse.json({ error: "Invalid JSON body" }, { status: 400 });
  }

  const targetLang =
    typeof body.to === "string"
      ? body.to
      : typeof body.lang === "string"
        ? body.lang
        : "fr";
  const skipRemote = body.skipRemote === true;

  if (Array.isArray(body.texts)) {
    const items = body.texts
      .filter((item): item is string => typeof item === "string")
      .slice(0, 50);
    if (items.length === 0) {
      return NextResponse.json(
        { error: "Empty `texts` array" },
        { status: 400 },
      );
    }
    const results = await Promise.all(
      items.map((text) => translateIevrQuery({ text, targetLang, skipRemote })),
    );
    return NextResponse.json(
      { targetLang, count: results.length, results },
      { headers: { "Cache-Control": "no-store" } },
    );
  }

  const text =
    typeof body.text === "string"
      ? body.text
      : typeof body.q === "string"
        ? body.q
        : "";
  if (!text.trim()) {
    return NextResponse.json(
      { error: "Missing `text` or `texts` field" },
      { status: 400 },
    );
  }

  const result = await translateIevrQuery({
    text,
    targetLang,
    skipRemote,
  });

  return NextResponse.json(result, {
    headers: { "Cache-Control": "no-store" },
  });
}
