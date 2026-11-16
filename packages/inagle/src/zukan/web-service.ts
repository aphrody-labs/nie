// SPDX-License-Identifier: MIT
/** Network acquisition for the zukan crawlers: Impit when installed, else an injected service. */
export interface WebResponse {
  status: number;
  truncated: boolean;
  body: string;
}

export interface ZukanWebService {
  response(url: string, options?: { maxBytes?: number }): Promise<WebResponse>;
  bytes(url: string): Promise<Uint8Array>;
}

export type ZukanWebServiceFactory = () =>
  ZukanWebService | Promise<ZukanWebService>;
let serviceFactory: ZukanWebServiceFactory | null = null;

/** Override the service for another host or tests; null restores the default (Impit). */
export function setZukanWebServiceFactory(
  factory: ZukanWebServiceFactory | null,
): void {
  serviceFactory = factory;
}

const runtimeImport = new Function("specifier", "return import(specifier)") as <
  T,
>(
  specifier: string,
) => Promise<T>;

async function createImpitZukanWebService(): Promise<ZukanWebService | null> {
  try {
    const { Impit } = await runtimeImport<typeof import("impit")>("impit");
    const client = new Impit({
      browser: "chrome",
      vanillaFallback: true,
      timeout: 45_000,
    });
    return {
      async response(url, options = {}) {
        const res = await client.fetch(url, { timeout: 45_000 });
        const body = await res.text();
        const bytes = Buffer.byteLength(body);
        return {
          status: res.status,
          truncated: options.maxBytes !== undefined && bytes > options.maxBytes,
          body,
        };
      },
      async bytes(url) {
        const res = await client.fetch(url, { timeout: 60_000 });
        if (!res.ok) throw new Error(`HTTP ${res.status} : ${url}`);
        return res.bytes();
      },
    };
  } catch {
    return null;
  }
}

export async function createZukanWebService(): Promise<ZukanWebService> {
  if (serviceFactory) return serviceFactory();
  const impitService = await createImpitZukanWebService();
  if (impitService) return impitService;
  throw new Error(
    "No Web service: install impit or call setZukanWebServiceFactory().",
  );
}

/** Require complete original HTML before domain parsing or writing source evidence. */
export async function zukanResponseHtml(
  service: ZukanWebService,
  url: string,
  context: string,
): Promise<string> {
  const response = await service.response(url, { maxBytes: 16_000_000 });
  if (response.status < 200 || response.status >= 300) {
    throw new Error(`${context}: HTTP ${response.status}`);
  }
  if (response.truncated) throw new Error(`${context}: truncated response`);
  return response.body;
}
