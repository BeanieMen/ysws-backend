import { NextRequest } from "next/server";
import { backendFetch, proxyResponse } from "@/lib/backend";

export async function GET(request: NextRequest) {
  return proxyResponse(await backendFetch(request, "/api/v1/reviews/projects"));
}
