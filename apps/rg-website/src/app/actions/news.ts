"use server";

import { createPostgresClient } from "@rosegriffon/db/service";

export interface PatchNoteSummary {
  id: string;
  slug: string;
  title: string;
  date: string;
  platform: string[];
  featured_image: string | null;
}

export interface PatchNoteDetail extends PatchNoteSummary {
  content_html: string;
}

function parsePatchDateScore(dateStr: string): number {
  const m = dateStr.match(/(\d{4})[./-](\d{1,2})[./-](\d{1,2})/);
  if (!m) return 0;
  return (
    Number.parseInt(m[1]!, 10) * 10_000 +
    Number.parseInt(m[2]!, 10) * 100 +
    Number.parseInt(m[3]!, 10)
  );
}

function parsePatchVersionScore(id: string): number {
  const m = id.match(/ver_(\d+)_(\d+)_(\d+)/i);
  if (!m) return 0;
  return (
    Number.parseInt(m[1]!, 10) * 10_000 +
    Number.parseInt(m[2]!, 10) * 100 +
    Number.parseInt(m[3]!, 10)
  );
}

/**
 * Récupère la liste des patch notes triés chronologiquement
 */
export async function getPatchNotes(
  platform?: string,
  limit?: number,
): Promise<PatchNoteSummary[]> {
  const supabase = createPostgresClient();

  const { data, error } = await supabase
    .from("patch_notes")
    .select("id, url, title, title_fr, date, featured_image, platform")
    .order("date", { ascending: false });

  if (error) {
    console.error("Error fetching patch notes:", error);
    return [];
  }

  let notes: PatchNoteSummary[] = ((data as any[]) || []).map((item) => ({
    date: item.date || "",
    featured_image: item.featured_image,
    id: item.id,
    platform: Array.isArray(item.platform) ? item.platform : [],
    slug: item.id,
    title: item.title_fr || item.title,
  }));

  notes.sort((a, b) => {
    const dateDiff = parsePatchDateScore(b.date) - parsePatchDateScore(a.date);
    if (dateDiff !== 0) return dateDiff;
    return parsePatchVersionScore(b.id) - parsePatchVersionScore(a.id);
  });

  if (platform) {
    const platformKey =
      platform === "ps-steam" ? ["ps4", "ps5", "steam"] : [platform];
    notes = notes.filter((note) =>
      note.platform.some((p) => platformKey.includes(p)),
    );
  }

  return limit ? notes.slice(0, limit) : notes;
}

/**
 * Récupère le détail d'un patch note par son ID ou slug
 */
export async function getPatchNoteDetail(
  idOrSlug: string,
): Promise<PatchNoteDetail | null> {
  const supabase = createPostgresClient();

  const { data, error } = await supabase
    .from("patch_notes")
    .select(
      "id, url, title, title_fr, date, featured_image, content_html, content_html_fr, platform",
    )
    .eq("id", idOrSlug)
    .single();

  if (error || !data) {
    console.error("Error fetching patch note detail:", error);
    return null;
  }

  const item = data as any;
  return {
    content_html: item.content_html_fr || item.content_html || "",
    date: item.date || "",
    featured_image: item.featured_image,
    id: item.id,
    platform: Array.isArray(item.platform) ? item.platform : [],
    slug: item.id,
    title: item.title_fr || item.title,
  };
}
