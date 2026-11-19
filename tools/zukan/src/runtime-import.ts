// SPDX-License-Identifier: MIT
/** CSP-safe dynamic loader for optional runtime packages. */
export function runtimeImport<T>(specifier: string): Promise<T> {
  return import(
    /* webpackIgnore: true */ /* @vite-ignore */ specifier
  ) as Promise<T>;
}
