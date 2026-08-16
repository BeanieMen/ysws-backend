import { NextRequest, NextResponse } from "next/server";
import { backendUrl } from "@/lib/backend";

export async function GET(request: NextRequest) {
  const url = new URL(request.url);
  const targetUrl = `${backendUrl}/auth/hackclub/login${url.search}`;
  return NextResponse.redirect(targetUrl);
}
