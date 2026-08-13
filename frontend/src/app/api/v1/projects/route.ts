import { NextRequest } from "next/server";
import { backendFetch, proxyResponse } from "@/lib/backend";

export async function GET(request: NextRequest) {
  return proxyResponse(await backendFetch(request, "/api/v1/projects"));
}

export async function POST(request: NextRequest) {
  const body = await request.text();
  return proxyResponse(await backendFetch(request, "/api/v1/projects", {
    method: "POST",
    headers: { "content-type": request.headers.get("content-type") || "application/json" },
    body,
  }));
}
