import { NextRequest } from "next/server";
import { backendFetch, proxyResponse } from "@/lib/backend";

export async function PUT(
  request: NextRequest,
  { params }: { params: Promise<{ project_id: string }> }
) {
  const { project_id } = await params;
  const body = await request.text();
  return proxyResponse(await backendFetch(request, `/api/v1/projects/${project_id}/hackatime-projects`, {
    method: "PUT",
    headers: { "content-type": request.headers.get("content-type") || "application/json" },
    body,
  }));
}
