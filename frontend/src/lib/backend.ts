import { NextRequest, NextResponse } from "next/server";

export const backendUrl = process.env.BACKEND_URL ?? "http://localhost:8000";


/**
 * Proxies only the headers the Rust API needs. In particular, it forwards the
 * browser's session cookie and never forwards the frontend Host header.
 */
export async function backendFetch(
  request: NextRequest,
  path: string,
  init: RequestInit = {},
) {
  const headers = new Headers(init.headers);
  const cookie = request.headers.get("cookie");
  if (cookie) headers.set("cookie", cookie);
  const origin = request.headers.get("origin");
  if (origin) headers.set("origin", origin);
  const contentType = request.headers.get("content-type");
  if (contentType && !headers.has("content-type")) headers.set("content-type", contentType);
  const idempotencyKey = request.headers.get("idempotency-key");
  if (idempotencyKey && !headers.has("idempotency-key")) headers.set("idempotency-key", idempotencyKey);

  return fetch(`${backendUrl}${path}`, {
    ...init,
    headers,
    cache: "no-store",
    redirect: "manual",
  });
}

export async function proxyResponse(response: globalThis.Response) {
  const body = response.status === 204 ? null : await response.text();
  const nextResponse = new NextResponse(body, { status: response.status });
  const contentType = response.headers.get("content-type");
  const location = response.headers.get("location");
  const setCookie = response.headers.get("set-cookie");
  if (contentType) nextResponse.headers.set("content-type", contentType);
  if (location) nextResponse.headers.set("location", location);
  if (setCookie) nextResponse.headers.set("set-cookie", setCookie);
  return nextResponse;
}
