import { NextResponse } from "next/server";
import { loadWebsiteLocaleDictionary } from "@rosegriffon/azalee/server/ecosystem";

export const runtime = "nodejs";
export const revalidate = 300;

export async function GET(
  request: Request,
  { params }: { params: Promise<{ lang: string }> },
) {
  const { lang } = await params;
  const { searchParams } = new URL(request.url);
  const payload = loadWebsiteLocaleDictionary(lang, searchParams.get("scope"));

  if (!payload) {
    return NextResponse.json(
      { error: "Unsupported locale", lang },
      { status: 404 },
    );
  }

  return NextResponse.json(payload, {
    headers: {
      "Cache-Control": "public, max-age=300, stale-while-revalidate=3600",
    },
  });
}
