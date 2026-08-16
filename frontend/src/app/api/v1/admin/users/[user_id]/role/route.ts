import { NextRequest } from "next/server";
import { backendFetch, proxyResponse } from "@/lib/backend";

export async function PUT(
  request: NextRequest,
  { params }: { params: Promise<{ user_id: string }> },
) {
  const { user_id } = await params;
  return proxyResponse(await backendFetch(request, `/api/v1/admin/users/${user_id}/role`, {
    method: "PUT",
    headers: { "content-type": request.headers.get("content-type") || "application/json" },
    body: await request.text(),
  }));
}
