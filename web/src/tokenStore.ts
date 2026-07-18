import type { LaunchConfig } from './hub';
export type TokenRecord = { id: string; admin: string; config: LaunchConfig; hash: string; createdAt: number };
const key = (admin: string) => `constella.tokens.${admin}`;

export function listTokens(admin: string): TokenRecord[] {
  try { return JSON.parse(localStorage.getItem(key(admin)) || '[]'); } catch { return []; }
}
export function saveToken(rec: TokenRecord): void {
  const all = listTokens(rec.admin).filter((t) => t.id !== rec.id);
  all.unshift(rec);
  localStorage.setItem(key(rec.admin), JSON.stringify(all));
}
export function getToken(admin: string, id: string): TokenRecord | undefined {
  return listTokens(admin).find((t) => t.id === id);
}
