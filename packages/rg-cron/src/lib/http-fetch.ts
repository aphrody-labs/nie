/** Acquisition HTTP directe (`fetch` de Bun) : HTML d'une page et réponse bornée en octets. */

export interface FetchHtmlOptions {
  timeoutSecs?: number;
  userAgent?: string;
}

export interface FetchResponseOptions extends FetchHtmlOptions {
  /** Taille maximale du corps lu ; au-delà la réponse est tronquée. */
  maxBytes?: number;
}

export interface HttpResponse {
  url: string;
  status: number;
  headers: Record<string, string>;
  bytes: number;
  truncated: boolean;
  body: string;
}

function init(options: FetchHtmlOptions): RequestInit {
  return {
    redirect: "follow",
    signal: AbortSignal.timeout((options.timeoutSecs ?? 30) * 1000),
    headers: options.userAgent ? { "user-agent": options.userAgent } : {},
  };
}

/** Corps texte d'une page ; lève sur un statut hors 2xx. */
export async function fetchHtml(
  url: string,
  options: FetchHtmlOptions = {},
): Promise<string> {
  const res = await fetch(url, init(options));
  if (!res.ok) throw new Error(`HTTP ${res.status} : ${url}`);
  return res.text();
}

/** Réponse HTTP dont le corps est lu jusqu'à `maxBytes` (flux annulé au-delà). */
export async function fetchResponse(
  url: string,
  options: FetchResponseOptions = {},
): Promise<HttpResponse> {
  const res = await fetch(url, init(options));
  const limit = options.maxBytes ?? Number.POSITIVE_INFINITY;
  const chunks: Uint8Array[] = [];
  let bytes = 0;
  let truncated = false;
  const reader = res.body?.getReader();
  while (reader) {
    const { done, value } = await reader.read();
    if (done) break;
    if (bytes + value.length > limit) {
      chunks.push(value.subarray(0, limit - bytes));
      bytes = limit;
      truncated = true;
      await reader.cancel();
      break;
    }
    chunks.push(value);
    bytes += value.length;
  }
  const body = new TextDecoder().decode(Buffer.concat(chunks));
  return {
    url: res.url || url,
    status: res.status,
    headers: Object.fromEntries(res.headers),
    bytes,
    truncated,
    body,
  };
}
