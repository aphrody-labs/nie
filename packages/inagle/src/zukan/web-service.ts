// SPDX-License-Identifier: MIT
/** Thin optional-peer binding; Aphrody Web owns all network acquisition. */
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
const APHRODY_WEB_MODULE = "@aphrody/agent-browser/service";
let serviceFactory: ZukanWebServiceFactory | null = null;

/** Override the service for another host or tests; null restores the optional peer. */
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
  let module: { WebService?: new () => ZukanWebService };
  try {
    const specifier = APHRODY_WEB_MODULE;
    module = await import(specifier);
  } catch (error) {
    throw new Error(
      `No Web service: install impit or ${APHRODY_WEB_MODULE}, or call setZukanWebServiceFactory().`,
      { cause: error },
    );
  }
  if (typeof module.WebService !== "function") {
    throw new Error(`${APHRODY_WEB_MODULE} does not export WebService`);
  }
  return new module.WebService();
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
