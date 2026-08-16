import { NextRequest } from "next/server";
import { backendFetch, proxyResponse } from "@/lib/backend";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ project_id: string }> },
) {
  const { project_id } = await params;
  return proxyResponse(await backendFetch(request, `/api/v1/projects/${project_id}/reviews`));
}

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ project_id: string }> },
) {
  const { project_id } = await params;
  return proxyResponse(await backendFetch(request, `/api/v1/projects/${project_id}/reviews`, {
    method: "POST",
    headers: { "content-type": request.headers.get("content-type") || "application/json" },
    body: await request.text(),
  }));
}
