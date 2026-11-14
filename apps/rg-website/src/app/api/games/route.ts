import { NextResponse } from "next/server";
import { queryGamesCatalog } from "@rosegriffon/inagle/games";

export const revalidate = 300;

export function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const payload = queryGamesCatalog({
    locale: searchParams.get("locale") ?? searchParams.get("lang"),
    era: searchParams.get("era"),
    status: searchParams.get("status"),
    q: searchParams.get("q"),
  });

  return NextResponse.json(payload, {
    headers: {
      "Cache-Control": "public, max-age=300, stale-while-revalidate=3600",
    },
  });
}
