import { NextResponse } from "next/server";
import { queryCommunityWikisEndpoint } from "@rosegriffon/azalee/server/ecosystem";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const rawLimit = Number.parseInt(searchParams.get("limit") ?? "50", 10);
  const limit = Number.isFinite(rawLimit) ? rawLimit : 50;

  const payload = queryCommunityWikisEndpoint({
    q: searchParams.get("q"),
    lang: searchParams.get("lang"),
    gameSlug: searchParams.get("game"),
    locale: searchParams.get("locale"),
    limit,
  });

  return NextResponse.json(payload, {
    headers: {
      "Cache-Control": "public, max-age=120, stale-while-revalidate=1800",
    },
  });
}
