import { NextRequest } from "next/server";
import { backendFetch, proxyResponse } from "@/lib/backend";

export async function GET(request: NextRequest) {
  const url = new URL(request.url);
  return proxyResponse(await backendFetch(request, `/auth/hackclub/login${url.search}`));
}

export async function POST(request: NextRequest) {
  return GET(request);
}
