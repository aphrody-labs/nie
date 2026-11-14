import { NextResponse } from "next/server";
import { queryCommunityModsEndpoint } from "@rosegriffon/azalee/server/ecosystem";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const rawLimit = Number.parseInt(searchParams.get("limit") ?? "50", 10);
  const limit = Number.isFinite(rawLimit) ? rawLimit : 50;

  const payload = queryCommunityModsEndpoint({
    q: searchParams.get("q"),
    gameSlug: searchParams.get("game"),
    locale: searchParams.get("locale") ?? searchParams.get("lang"),
    limit,
  });

  return NextResponse.json(payload, {
    headers: {
      "Cache-Control": "public, max-age=120, stale-while-revalidate=1800",
    },
  });
}
