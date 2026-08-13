import { NextRequest } from "next/server";
import { backendFetch, proxyResponse } from "@/lib/backend";

export async function POST(request: NextRequest) {
  return proxyResponse(await backendFetch(request, "/auth/logout", {
    method: "POST",
  }));
}
