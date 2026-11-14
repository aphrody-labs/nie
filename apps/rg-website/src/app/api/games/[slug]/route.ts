import { NextResponse } from "next/server";
import { queryGameDetail } from "@rosegriffon/inagle/games";

export const revalidate = 300;

export async function GET(
  request: Request,
  { params }: { params: Promise<{ slug: string }> },
) {
  const { slug } = await params;
  const { searchParams } = new URL(request.url);
  const detail = queryGameDetail(
    slug,
    searchParams.get("locale") ?? searchParams.get("lang"),
  );

  if (!detail) {
    return NextResponse.json(
      { error: "Game not found", slug },
      { status: 404 },
    );
  }

  return NextResponse.json(detail, {
    headers: {
      "Cache-Control": "public, max-age=300, stale-while-revalidate=3600",
    },
  });
}
